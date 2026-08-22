//! DA download, cache, and verification.
//!
//! DA blobs are downloaded from the manifest's raw URL, SHA-256 verified, and
//! atomically written into `penumbra_dir()/da/<brand>-<chipset>.bin`. A failed
//! or corrupt download never leaves a partial file at the final path.

use crate::penumbra::manifest::DAEntry;
use crate::penumbra::{PenumbraError, Result, penumbra_dir};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Read chunk size for streaming a download with progress.
const READ_CHUNK: usize = 64 * 1024;

/// The on-disk path for a cached DA blob.
#[must_use]
pub fn da_cache_path(brand: &str, chipset: &str) -> PathBuf {
    penumbra_dir().join("da").join(format!("{brand}-{chipset}.bin"))
}

/// SHA-256 hex digest of `bytes`.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
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

/// Download and cache a DA entry, verifying its SHA-256.
///
/// # Errors
///
/// Returns [`PenumbraError::Download`] on HTTP failure and
/// [`PenumbraError::HashMismatch`] on a bad digest.
pub fn download_da(
    entry: &DAEntry,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<PathBuf> {
    let bytes = download_da_bytes(entry, on_progress)?;
    let root = penumbra_dir().join("da");
    write_da_bytes(entry, &root, &bytes)
}

/// Download the DA blob into memory (blocking), reporting progress per chunk.
///
/// # Errors
///
/// Returns [`PenumbraError::Download`] on any HTTP failure.
fn download_da_bytes(entry: &DAEntry, on_progress: &mut dyn FnMut(u64, u64)) -> Result<Vec<u8>> {
    let mut res = ureq::get(&entry.url)
        .call()
        .map_err(|source| PenumbraError::Download { url: entry.url.clone(), source })?;
    let total = res.body_mut().content_length().unwrap_or(0);
    let mut reader = res.body_mut().as_reader();
    let mut buf = vec![0u8; READ_CHUNK];
    let mut out = Vec::new();
    let mut done = 0u64;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|source| PenumbraError::Download { url: entry.url.clone(), source: ureq::Error::Io(source) })?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
        done += u64::try_from(n).unwrap_or(u64::MAX);
        on_progress(done, total);
    }
    Ok(out)
}

/// Verify, write, and atomically cache the DA blob `bytes` for `entry`.
///
/// This is the network-free core of the cache path (tests drive it directly
/// with local bytes). Staging happens in the cache dir's parent so the final
/// rename stays on one filesystem.
///
/// # Errors
///
/// Returns [`PenumbraError::HashMismatch`] on a bad digest and
/// [`PenumbraError::Cache`] on write failure.
fn write_da_bytes(entry: &DAEntry, root: &Path, bytes: &[u8]) -> Result<PathBuf> {
    let actual = sha256_hex(bytes);
    if actual != entry.sha256 {
        return Err(PenumbraError::HashMismatch {
            expected: entry.sha256.clone(),
            actual,
        });
    }

    let parent = root.parent().ok_or_else(|| PenumbraError::Cache("cache root has no parent".into()))?;
    fs::create_dir_all(parent).map_err(|source| PenumbraError::Cache(source.to_string()))?;
    fs::create_dir_all(root).map_err(|source| PenumbraError::Cache(source.to_string()))?;

    let final_path = root.join(format!("{}-{}.bin", entry.brand, entry.chipset));
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let rand_suffix: u32 = rand::random();
    let staging = parent.join(format!(".da-stage-{stamp}-{rand_suffix}"));
    let _guard = StageCleanup(staging.clone());
    fs::write(&staging, bytes).map_err(|source| PenumbraError::Cache(source.to_string()))?;
    fs::rename(&staging, &final_path).map_err(|source| PenumbraError::Cache(source.to_string()))?;

    Ok(final_path)
}

/// Verify a DA blob on disk against an expected SHA-256.
///
/// # Errors
///
/// Returns [`PenumbraError::Cache`] on read failure and
/// [`PenumbraError::HashMismatch`] on a bad digest.
pub fn verify_da(path: &Path, sha256: &str) -> Result<()> {
    let bytes = fs::read(path).map_err(|source| PenumbraError::Cache(source.to_string()))?;
    let actual = sha256_hex(&bytes);
    if actual != sha256 {
        return Err(PenumbraError::HashMismatch { expected: sha256.to_string(), actual });
    }
    Ok(())
}

/// Remove all cached DA blobs under `penumbra_dir()/da/`.
///
/// # Errors
///
/// Returns [`PenumbraError::Cache`] on filesystem failure.
pub fn remove_cached_da() -> Result<()> {
    remove_cached_da_at(&penumbra_dir().join("da"))
}

/// Testable core of [`remove_cached_da`] with an explicit cache dir.
///
/// # Errors
///
/// Returns [`PenumbraError::Cache`] on filesystem failure.
fn remove_cached_da_at(dir: &Path) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    fs::remove_dir_all(dir).map_err(|source| PenumbraError::Cache(source.to_string()))
}

/// Removes the staging file on drop (best-effort).
struct StageCleanup(PathBuf);

impl Drop for StageCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::penumbra::manifest::DAEntry;

    const BLOB: &[u8] = b"fake-da-blob";
    const EXPECTED_SHA: &str = "b342204f71b1e3525e747bb694d075a3e22452806e1f3a571463d91d043a9688";

    fn entry() -> DAEntry {
        DAEntry {
            brand: "infinix".into(),
            chipset: "mt6789".into(),
            devices: vec!["Infinix NOTE 12".into()],
            url: "https://example.invalid/DA/infinix/mt6789.bin".into(),
            sha256: EXPECTED_SHA.into(),
        }
    }

    #[test]
    fn sha256_hex_matches_expectation() {
        assert_eq!(sha256_hex(BLOB), EXPECTED_SHA);
    }

    #[test]
    fn write_da_bytes_caches_to_brand_chipset_path() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("da");
        let path = write_da_bytes(&entry(), &root, BLOB).unwrap();
        assert_eq!(path, root.join("infinix-mt6789.bin"));
        assert_eq!(fs::read(&path).unwrap(), BLOB);
        assert!(tmp.path().read_dir().unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().starts_with(".da-stage-")));
    }

    #[test]
    fn write_da_bytes_rejects_corrupt_blob() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("da");
        let err = write_da_bytes(&entry(), &root, b"tampered").unwrap_err();
        assert!(matches!(err, PenumbraError::HashMismatch { .. }));
        assert!(!root.exists() || root.read_dir().unwrap().next().is_none());
    }

    #[test]
    fn verify_da_accepts_and_rejects() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("da.bin");
        fs::write(&path, BLOB).unwrap();
        assert!(verify_da(&path, EXPECTED_SHA).is_ok());
        assert!(matches!(
            verify_da(&path, "a".repeat(64).as_str()),
            Err(PenumbraError::HashMismatch { .. })
        ));
    }

    #[test]
    fn remove_cached_da_clears_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("da");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("infinix-mt6789.bin"), BLOB).unwrap();
        remove_cached_da_at(&dir).unwrap();
        assert!(!dir.exists());
        assert!(remove_cached_da_at(&dir).is_ok());
    }
}
