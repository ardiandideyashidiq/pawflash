//! Release manifest fetch and platform resolution.

use crate::mtk::{MtkError, Result};
use serde::Deserialize;
use std::collections::HashMap;

/// Fixed consumer URL for the mtk bridge release manifest. Everything else
/// (tag, asset names, hashes) is resolved from this document.
pub const FIXED_MANIFEST_URL: &str =
    "https://github.com/ardiandideyashidiq/mtkclient/releases/latest/download/manifest.json";

/// One platform's bridge asset.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PlatformAsset {
    pub url: String,
    pub sha256: String,
}

/// The bridge release manifest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub commit: String,
    pub platforms: HashMap<String, PlatformAsset>,
}

impl Manifest {
    /// Resolve the asset for a platform key.
    ///
    /// # Errors
    ///
    /// Returns [`MtkError::MissingAsset`] if the platform is absent from the
    /// manifest.
    pub fn asset_for(&self, platform: &str) -> Result<&PlatformAsset> {
        self.platforms
            .get(platform)
            .ok_or_else(|| MtkError::MissingAsset { platform: platform.to_string() })
    }
}

/// The manifest platform key for the current host.
///
/// # Errors
///
/// Returns [`MtkError::UnsupportedPlatform`] for non-`linux-x86_64` /
/// `windows-x86_64` hosts.
pub fn current_platform() -> Result<String> {
    let os = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        return Err(MtkError::UnsupportedPlatform(std::env::consts::OS.to_string()));
    };
    let arch = if cfg!(target_arch = "x86_64") { "x86_64" } else { return Err(MtkError::UnsupportedPlatform(std::env::consts::ARCH.to_string())) };
    Ok(format!("{os}-{arch}"))
}

/// Fetch and parse the release manifest.
///
/// # Errors
///
/// Returns [`MtkError::ManifestFetch`] on network failure or malformed JSON.
pub fn fetch_manifest() -> Result<Manifest> {
    let mut res = ureq::get(FIXED_MANIFEST_URL)
        .call()
        .map_err(|source| MtkError::ManifestFetch(source.to_string()))?;
    let body = res
        .body_mut()
        .read_to_string()
        .map_err(|source| MtkError::ManifestFetch(source.to_string()))?;
    serde_json::from_str(&body).map_err(|source| MtkError::ManifestFetch(source.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL_MANIFEST: &str = r#"{
      "version": "9fde5ca",
      "commit": "9fde5ca10f070350eee6efa136ca5d7acb38a9f7",
      "platforms": {
        "linux-x86_64": {
          "url": "https://github.com/ardiandideyashidiq/mtkclient/releases/download/bridge-9fde5ca/pawflash-mtkbridge-linux-x86_64.tar.gz",
          "sha256": "1e983d17c36a79a4d0342800d37631fe3556a6099d6177e6d38c230f5bbfed6a"
        },
        "windows-x86_64": {
          "url": "https://github.com/ardiandideyashidiq/mtkclient/releases/download/bridge-9fde5ca/pawflash-mtkbridge-windows-x86_64.tar.gz",
          "sha256": "f49752e4e97f1f5cb2de4e45a84a9ecf9f04e381dbeac6203b398dacb63ccf41"
        }
      }
    }"#;

    #[test]
    fn parses_real_manifest() {
        let m: Manifest = serde_json::from_str(REAL_MANIFEST).unwrap();
        assert_eq!(m.version, "9fde5ca");
        assert_eq!(m.platforms.len(), 2);
    }

    #[test]
    fn asset_for_resolves() {
        let m: Manifest = serde_json::from_str(REAL_MANIFEST).unwrap();
        let a = m.asset_for("linux-x86_64").unwrap();
        assert!(a.url.ends_with("pawflash-mtkbridge-linux-x86_64.tar.gz"));
    }

    #[test]
    fn asset_for_unknown_errors() {
        let m: Manifest = serde_json::from_str(REAL_MANIFEST).unwrap();
        assert!(m.asset_for("darwin").is_err());
    }

    #[test]
    fn current_platform_matches_host() {
        let p = current_platform().unwrap();
        assert!(p == "linux-x86_64" || p == "windows-x86_64");
    }
}
