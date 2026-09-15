//! DA manifest fetch, cache, and device-name resolution.
//!
//! The penumbra fork publishes a `DA/manifest.json` listing every hosted
//! `DA/<brand>/<chipset>.bin` blob with its retail `devices`, sha256 and raw
//! URL. Consumers resolve a DA by device name (primary) or `(brand, chipset)`.
//! Local caching and custom DA additions are supported for offline/custom use.

pub mod cache;
pub mod resolve;
pub mod types;

use crate::penumbra::{PenumbraError, Result};
pub use cache::{
    custom_da_path, load_cached_manifest, load_custom_dais, manifest_cache_path,
    save_cached_manifest, save_custom_da,
};
pub use resolve::{resolve_by_brand_chipset_in, resolve_by_device_in};
pub use types::{DAEntry, DAManifest, FileBlob};

/// Fixed consumer URL for the DA manifest. Everything else (DA file paths,
/// URLs, hashes) is resolved from this document.
pub const DA_MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/ardiandideyashidiq/penumbra/main/DA/manifest.json";

/// Fetch and parse the DA manifest.
///
/// If network is reachable, downloads and saves to the local disk cache.
/// If offline or on network failure, falls back to the locally cached manifest.
///
/// # Errors
///
/// Returns [`PenumbraError::ManifestFetch`] on network failure when no cached manifest exists.
pub fn fetch_da_manifest() -> Result<DAManifest> {
    match ureq::get(DA_MANIFEST_URL).call() {
        Ok(mut res) => {
            let body = res
                .body_mut()
                .read_to_string()
                .map_err(|source| PenumbraError::ManifestFetch(source.to_string()))?;
            let manifest: DAManifest = serde_json::from_str(&body)
                .map_err(|source| PenumbraError::ManifestFetch(source.to_string()))?;
            let _ = save_cached_manifest(&manifest);
            Ok(manifest)
        }
        Err(err) => {
            if let Some(cached) = load_cached_manifest() {
                tracing::warn!(error = %err, "remote manifest fetch failed; using cached manifest");
                Ok(cached)
            } else {
                Err(PenumbraError::ManifestFetch(err.to_string()))
            }
        }
    }
}

/// Fetch the manifest and return its entries merged with any local custom DAs.
///
/// # Errors
///
/// Returns [`PenumbraError::ManifestFetch`] on network failure when no cached manifest exists.
pub fn list_dais() -> Result<Vec<DAEntry>> {
    let manifest = fetch_da_manifest()?;
    let mut dais = manifest.dais;
    merge_custom_dais(&mut dais);
    Ok(dais)
}

/// Fast, non-blocking check: returns cached manifest entries merged with any custom DAs.
#[must_use]
pub fn list_dais_cached() -> Vec<DAEntry> {
    let mut dais = load_cached_manifest().map_or_else(Vec::new, |m| m.dais);
    merge_custom_dais(&mut dais);
    dais
}

/// Merges user-defined custom DAs into `dais`, updating existing entries or appending.
fn merge_custom_dais(dais: &mut Vec<DAEntry>) {
    let custom = load_custom_dais();
    for c in custom {
        if let Some(idx) = dais.iter().position(|e| {
            if let (Some(id_a), Some(id_b)) = (&e.id, &c.id) {
                id_a.eq_ignore_ascii_case(id_b)
            } else {
                e.brand.eq_ignore_ascii_case(&c.brand) && e.chipset.eq_ignore_ascii_case(&c.chipset)
            }
        }) {
            dais[idx] = c;
        } else {
            dais.push(c);
        }
    }
}

/// Resolve a DA by retail device name.
///
/// # Errors
///
/// Returns [`PenumbraError::NoSuchDa`] (with close-match hints) when nothing matches.
pub fn resolve_by_device(query: &str) -> Result<DAEntry> {
    let dais = list_dais()?;
    resolve_by_device_in(&dais, query)
}

/// Resolve a DA by exact `(brand, chipset)`.
///
/// # Errors
///
/// Returns [`PenumbraError::NoSuchDa`] when no entry matches.
pub fn resolve_by_brand_chipset(brand: &str, chipset: &str) -> Result<DAEntry> {
    let dais = list_dais()?;
    resolve_by_brand_chipset_in(&dais, brand, chipset)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "version": "20194f5",
      "dais": [
        {
          "brand": "infinix",
          "chipset": "mt6789",
          "devices": ["Infinix NOTE 12", "Infinix NOTE 12i"],
          "url": "https://raw.githubusercontent.com/ardiandideyashidiq/penumbra/main/DA/infinix/mt6789.bin",
          "sha256": "3c7de4ee52b47f1d4c5122868b52dfa06c18e5ef940f4c8a04c46365a696bbdd"
        },
        {
          "brand": "xiaomi",
          "chipset": "mt6893",
          "devices": ["POCO X7 Pro"],
          "url": "https://raw.githubusercontent.com/ardiandideyashidiq/penumbra/main/DA/xiaomi/mt6893.bin",
          "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        }
      ]
    }"#;

    fn fixture() -> Vec<DAEntry> {
        let m: DAManifest = serde_json::from_str(FIXTURE).unwrap();
        m.dais
    }

    #[test]
    fn parses_manifest_shape() {
        let m: DAManifest = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(m.version, "20194f5");
        assert_eq!(m.dais.len(), 2);
        assert_eq!(m.dais[0].devices, vec!["Infinix NOTE 12", "Infinix NOTE 12i"]);
        assert_eq!(m.dais[0].sha256.len(), 64);
        assert!(!m.dais[0].has_auth());
    }

    #[test]
    fn parses_da_auth_combo() {
        let json = r#"{
          "version": "1.2.0",
          "dais": [
            {
              "id": "xiaomi-combo",
              "brand": "xiaomi",
              "chipset": "mt6877",
              "devices": ["Redmi Note 12 Pro 5G"],
              "da": {
                "url": "https://example.com/da.bin",
                "sha256": "3c7de4ee52b47f1d4c5122868b52dfa06c18e5ef940f4c8a04c46365a696bbdd"
              },
              "auth": {
                "url": "https://example.com/auth.auth",
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
              }
            }
          ]
        }"#;
        let m: DAManifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.dais.len(), 1);
        let entry = &m.dais[0];
        assert!(entry.has_auth());
        assert_eq!(entry.da_url(), "https://example.com/da.bin");
        assert_eq!(entry.auth_url(), Some("https://example.com/auth.auth"));
    }

    #[test]
    fn resolve_by_device_matches_substring_case_insensitive() {
        let e = resolve_by_device_in(&fixture(), "note 12").unwrap();
        assert_eq!(e.brand, "infinix");
        assert_eq!(e.chipset, "mt6789");
    }

    #[test]
    fn resolve_by_device_matches_second_entry() {
        let e = resolve_by_device_in(&fixture(), "POCO X7").unwrap();
        assert_eq!(e.brand, "xiaomi");
        assert_eq!(e.chipset, "mt6893");
    }

    #[test]
    fn resolve_by_device_no_hit_has_hints() {
        let err = resolve_by_device_in(&fixture(), "galaxy").unwrap_err();
        assert!(err.to_string().contains("did you mean"));
        assert!(err.to_string().contains("Infinix NOTE 12"));
    }

    #[test]
    fn resolve_by_brand_chipset_exact() {
        let e = resolve_by_brand_chipset_in(&fixture(), "infinix", "mt6789").unwrap();
        assert_eq!(e.chipset, "mt6789");
    }

    #[test]
    fn resolve_by_brand_chipset_case_insensitive() {
        let e = resolve_by_brand_chipset_in(&fixture(), "Infinix", "MT6789").unwrap();
        assert_eq!(e.brand, "infinix");
    }

    #[test]
    fn resolve_by_brand_chipset_unknown_errors() {
        let err = resolve_by_brand_chipset_in(&fixture(), "samsung", "exynos").unwrap_err();
        assert!(err.to_string().contains("samsung/exynos"));
    }

    #[test]
    fn entry_label_uses_first_device() {
        let e = resolve_by_brand_chipset_in(&fixture(), "infinix", "mt6789").unwrap();
        assert_eq!(e.label(), "Infinix NOTE 12  (infinix · mt6789)");
    }

    #[test]
    fn custom_da_merge_and_override() {
        let mut dais = fixture();
        let custom = DAEntry {
            id: Some("custom-mt6789".into()),
            brand: "infinix".into(),
            chipset: "mt6789".into(),
            devices: vec!["Custom Infinix Phone".into()],
            url: "https://example.com/custom.bin".into(),
            sha256: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            is_custom: Some(true),
            ..Default::default()
        };
        // Merging custom entry with same brand/chipset should override
        let custom_list = vec![custom];
        for c in custom_list {
            if let Some(idx) = dais.iter().position(|e| {
                e.brand.eq_ignore_ascii_case(&c.brand) && e.chipset.eq_ignore_ascii_case(&c.chipset)
            }) {
                dais[idx] = c;
            } else {
                dais.push(c);
            }
        }
        assert_eq!(dais.len(), 2);
        assert_eq!(dais[0].devices[0], "Custom Infinix Phone");
        assert!(dais[0].is_custom_entry());
    }
}
