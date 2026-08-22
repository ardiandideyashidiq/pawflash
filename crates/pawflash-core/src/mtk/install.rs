//! Download, verify, and atomic install of the frozen mtk bridge.
//!
//! The bridge archive is downloaded from the release manifest, SHA-256
//! verified, and extracted into a staging directory before being renamed into
//! place, so a failed download never leaves a partial install.

use crate::mtk::error::MtkError;
use crate::mtk::Manifest;
use crate::penumbra::platform::base_data_dir;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Read chunk size for streaming a download with progress.
const READ_CHUNK: usize = 64 * 1024;

/// The platform data directory holding the installed bridge.
#[must_use]
pub fn install_root() -> PathBuf {
    base_data_dir().join("mtk-bridge")
}

/// Read the version file at `root`; `None` if absent.
fn read_version(root: &Path) -> Option<String> {
    fs::read_to_string(root.join("version")).ok().map(|s| s.trim().to_string())
}

/// Installed bridge version at the default install root, if any.
#[must_use]
pub fn current_version() -> Option<String> {
    read_version(&install_root())
}

/// Installed bridge version under an explicit root (test seam).
fn current_version_at(root: &Path) -> Option<String> {
    read_version(root)
}

/// Read `reader` to completion, invoking `on_progress(done, total)` per chunk.
///
/// `total` is a hint from the HTTP `Content-Length`; when unknown it is `0`
/// and callers should treat progress as indeterminate.
fn read_body_with_progress(
    mut reader: impl Read,
    total: u64,
    on_progress: &mut dyn FnMut(u64, u64),
) -> std::io::Result<Vec<u8>> {
    let mut buf = vec![0u8; READ_CHUNK];
    let mut out = Vec::new();
    let mut done = 0u64;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
        done += u64::try_from(n).unwrap_or(u64::MAX);
        on_progress(done, total);
    }
    Ok(out)
}

/// Download `url` into memory (blocking), reporting progress per 64 KiB chunk.
///
/// # Errors
///
/// Returns [`MtkError::Download`] on any HTTP failure.
pub fn download_bytes(
    url: &str,
    on_progress: Option<&mut dyn FnMut(u64, u64)>,
) -> crate::mtk::Result<Vec<u8>> {
    let mut res = ureq::get(url)
        .call()
        .map_err(|source| MtkError::Download { url: url.to_string(), source })?;
    let total = res.body_mut().content_length().unwrap_or(0);
    match on_progress {
        Some(cb) => read_body_with_progress(res.body_mut().as_reader(), total, cb)
            .map_err(|source| MtkError::Download {
                url: url.to_string(),
                source: ureq::Error::Io(source),
            }),
        None => res
            .body_mut()
            .read_to_vec()
            .map_err(|source| MtkError::Download { url: url.to_string(), source }),
    }
}

/// SHA-256 hex digest of `bytes`.
fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Extract the archive `bytes` (a gzipped tar rooted at `bridge/`) into `root`.
fn extract_archive(bytes: &[u8], root: &Path) -> Result<(), MtkError> {
    let decoder = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(root)
        .map_err(|source| MtkError::Extract(source.to_string()))?;
    Ok(())
}

/// Verify, extract, and atomically install archive `bytes` for `manifest`.
///
/// This is the network-free core of the install path (tests drive it directly
/// with locally-generated archives). Staging happens in a sibling dir of
/// `root` so the final rename stays on one filesystem (a `tempdir()` in `/tmp`
/// would cross devices when `root` lives on the home mount, and
/// `fs::rename` would fail with `EXDEV`).
///
/// # Errors
///
/// Returns a [`MtkError`] on hash mismatch or extract/move failure.
fn install_from_bytes(manifest: &Manifest, root: &Path, bytes: &[u8]) -> crate::mtk::Result<PathBuf> {
    let asset = manifest.asset_for(&crate::mtk::manifest::current_platform()?)?;

    let actual = sha256_hex(bytes);
    if actual != asset.sha256 {
        return Err(MtkError::HashMismatch { expected: asset.sha256.clone(), actual });
    }

    let parent = root.parent().ok_or_else(|| MtkError::Install("install root has no parent".into()))?;
    fs::create_dir_all(parent).map_err(|source| MtkError::Install(source.to_string()))?;
    fs::create_dir_all(root).map_err(|source| MtkError::Install(source.to_string()))?;

    // Unique staging dir on the same filesystem as `root`.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let rand_suffix: u32 = rand::random();
    let staging = parent.join(format!(".mtk-bridge-stage-{stamp}-{rand_suffix}"));
    let _guard = StageCleanup(staging.clone());
    fs::create_dir_all(&staging).map_err(|source| MtkError::Install(source.to_string()))?;
    extract_archive(bytes, &staging)?;

    // Remove any previous install atomically-ish: rename the fresh one in,
    // then drop the old tree.
    let final_dir = root.join("bridge");
    let old_dir = root.join("bridge.old");
    if final_dir.exists() {
        let _ = fs::rename(&final_dir, &old_dir);
    }
    if staging.join("bridge").exists() {
        let staged = staging.join("bridge");
        fs::rename(&staged, &final_dir).map_err(|source| MtkError::Install(source.to_string()))?;
    } else {
        // Tolerate archives that don't wrap in `bridge/`.
        fs::create_dir_all(&final_dir).map_err(|source| MtkError::Install(source.to_string()))?;
        for entry in fs::read_dir(&staging)
            .map_err(|source| MtkError::Install(source.to_string()))?
        {
            let entry = entry.map_err(|source| MtkError::Install(source.to_string()))?;
            let target = final_dir.join(entry.file_name());
            fs::rename(entry.path(), &target)
                .map_err(|source| MtkError::Install(source.to_string()))?;
        }
    }
    let _ = fs::remove_dir_all(&old_dir);

    fs::write(root.join("version"), &manifest.version)
        .map_err(|source| MtkError::Install(source.to_string()))?;

    Ok(bridge_binary_path(root))
}

/// Removes the staging dir on drop (best-effort).
struct StageCleanup(PathBuf);

impl Drop for StageCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Ensure the bridge for `manifest` is installed under an explicit root.
///
/// Skips install when `root/version` already matches the manifest version.
/// `on_progress` receives `(downloaded_bytes, total_bytes)` during the
/// download phase (`total` is `0` when the server omits `Content-Length`).
///
/// # Errors
///
/// Returns a [`MtkError`] on download, hash, or extract failure.
pub fn ensure_installed_at(
    manifest: &Manifest,
    root: &Path,
    on_progress: Option<&mut dyn FnMut(u64, u64)>,
) -> crate::mtk::Result<PathBuf> {
    if current_version_at(root).as_deref() == Some(manifest.version.as_str()) {
        return Ok(bridge_binary_path(root));
    }

    let asset = manifest.asset_for(&crate::mtk::manifest::current_platform()?)?;
    let bytes = download_bytes(&asset.url, on_progress)?;
    install_from_bytes(manifest, root, &bytes)
}

/// Path to the bridge executable under `root`.
fn bridge_binary_path(root: &Path) -> PathBuf {
    let exe = if cfg!(target_os = "windows") { "bridge.exe" } else { "bridge" };
    root.join("bridge").join(exe)
}

/// Ensure the bridge for `manifest` is installed; return the binary path.
///
/// `on_progress` receives `(downloaded_bytes, total_bytes)` during the
/// download phase (`total` is `0` when the server omits `Content-Length`).
///
/// # Errors
///
/// Returns a [`MtkError`] on download, hash, or extract failure.
pub fn ensure_installed(
    manifest: &Manifest,
    on_progress: Option<&mut dyn FnMut(u64, u64)>,
) -> crate::mtk::Result<PathBuf> {
    let root = install_root();
    ensure_installed_at(manifest, &root, on_progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny valid tar.gz rooted at `bridge/` containing a `bridge` file.
    /// The payload is `/bin/sh`-ish text so the test can assert extraction.
    fn sample_archive_bytes() -> Vec<u8> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut builder = tar::Builder::new(cursor);
        let content = b"#!/bin/sh\necho fake bridge\n";
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o755);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        builder.append_data(&mut header, "bridge/bridge", &content[..]).unwrap();
        let cursor = builder.into_inner().unwrap();
        let inner = cursor.into_inner();

        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut encoder, &inner).unwrap();
        encoder.finish().unwrap()
    }

    fn sample_manifest_sha256() -> String {
        sha256_hex(&sample_archive_bytes())
    }

    fn make_manifest(version: &str, sha256: String, platform: Option<String>) -> Manifest {
        Manifest {
            version: version.into(),
            commit: "0".repeat(40),
            platforms: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    platform.unwrap_or_else(|| crate::mtk::manifest::current_platform().unwrap()),
                    crate::mtk::manifest::PlatformAsset {
                        url: "https://example.invalid/x.tar.gz".into(),
                        sha256,
                    },
                );
                m
            },
        }
    }

    #[test]
    fn installs_fresh_and_writes_version() {
        let tmp = tempfile::tempdir().unwrap();
        let manifest = make_manifest("v1", sample_manifest_sha256(), None);
        let bytes = sample_archive_bytes();
        let bin = install_from_bytes(&manifest, tmp.path(), &bytes).unwrap();
        assert!(bin.ends_with("bridge/bridge"));
        assert!(bin.exists());
        assert_eq!(read_version(tmp.path()).as_deref(), Some("v1"));
    }

    #[test]
    fn install_creates_missing_root_dir() {
        // Regression: the real install root (e.g. ~/.local/share/pawflash/mtk-bridge)
        // does not exist on first run; staging lives in the root's parent so the
        // final rename must create `root` first, else it fails with ENOENT.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("mtk-bridge");
        assert!(!root.exists());
        let manifest = make_manifest("v1", sample_manifest_sha256(), None);
        let bytes = sample_archive_bytes();
        let bin = install_from_bytes(&manifest, &root, &bytes).unwrap();
        assert!(bin.exists());
        assert_eq!(read_version(&root).as_deref(), Some("v1"));
    }

    #[test]
    fn stale_version_reinstalls() {
        let tmp = tempfile::tempdir().unwrap();
        let m1 = make_manifest("v1", sample_manifest_sha256(), None);
        let bytes = sample_archive_bytes();
        install_from_bytes(&m1, tmp.path(), &bytes).unwrap();

        // "v2" points at a different payload — same bytes, new version tag.
        let m2 = make_manifest("v2", sample_manifest_sha256(), None);
        let bin = install_from_bytes(&m2, tmp.path(), &bytes).unwrap();
        assert!(bin.exists());
        assert_eq!(read_version(tmp.path()).as_deref(), Some("v2"));
    }

    #[test]
    fn hash_mismatch_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let manifest = make_manifest("v1", "0".repeat(64), None);
        let bytes = sample_archive_bytes();
        let err = install_from_bytes(&manifest, tmp.path(), &bytes).unwrap_err();
        assert!(matches!(err, MtkError::HashMismatch { .. }));
        // Nothing installed.
        assert!(!tmp.path().join("bridge").exists());
    }

    #[test]
    fn up_to_date_skips_reinstall() {
        let tmp = tempfile::tempdir().unwrap();
        let manifest = make_manifest("v1", sample_manifest_sha256(), None);
        let bytes = sample_archive_bytes();
        install_from_bytes(&manifest, tmp.path(), &bytes).unwrap();
        let bin = ensure_installed_at(&manifest, tmp.path(), None).unwrap();
        assert!(bin.exists());
        assert_eq!(read_version(tmp.path()).as_deref(), Some("v1"));
    }

    #[test]
    fn download_bytes_fails_on_bad_url() {
        let err = download_bytes("https://example.invalid/does-not-exist.tar.gz", None);
        assert!(err.is_err());
    }

    #[test]
    fn read_body_with_progress_reports_cumulative_chunks() {
        // 200 KiB payload read in 64 KiB chunks → callbacks at 64/128/192/200 KiB.
        let payload = vec![0xABu8; 200 * 1024];
        let mut progress = Vec::new();
        let out = read_body_with_progress(
            std::io::Cursor::new(&payload),
            200 * 1024,
            &mut |done, total| progress.push((done, total)),
        )
        .expect("read succeeds");
        assert_eq!(out, payload);
        assert_eq!(progress, vec![
            (64 * 1024, 200 * 1024),
            (128 * 1024, 200 * 1024),
            (192 * 1024, 200 * 1024),
            (200 * 1024, 200 * 1024),
        ]);
    }

    #[test]
    fn read_body_with_progress_zero_total_still_reports() {
        // Unknown total (0) must not suppress progress reporting.
        let payload = vec![0u8; 10];
        let mut progress = Vec::new();
        let out = read_body_with_progress(
            std::io::Cursor::new(&payload),
            0,
            &mut |done, total| progress.push((done, total)),
        )
        .expect("read succeeds");
        assert_eq!(out, payload);
        assert_eq!(progress, vec![(10, 0)]);
    }

    #[test]
    fn sha256_hex_matches() {
        let hex = sha256_hex(b"hello");
        assert_eq!(
            hex,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }
}
