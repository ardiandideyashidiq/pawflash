//! DA manifest fetch and device-name resolution.
//!
//! The penumbra fork publishes a `DA/manifest.json` listing every hosted
//! `DA/<brand>/<chipset>.bin` blob with its retail `devices`, sha256 and raw
//! URL. Consumers resolve a DA by device name (primary) or `(brand, chipset)`.

use crate::penumbra::{PenumbraError, Result};
use serde::{Deserialize, Serialize};

/// Fixed consumer URL for the DA manifest. Everything else (DA file paths,
/// URLs, hashes) is resolved from this document.
pub const DA_MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/ardiandideyashidiq/penumbra/main/DA/manifest.json";

/// Metadata for a downloadable binary blob (DA or Auth).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileBlob {
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<u64>,
}

/// One hosted DA blob and optional companion auth blob.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DAEntry {
    #[serde(default)]
    pub id: Option<String>,
    /// OEM/brand subdirectory, e.g. `infinix`.
    pub brand: String,
    /// `SoC` chipset, e.g. `mt6789`.
    pub chipset: String,
    /// Retail device names using this DA, e.g. `["Infinix NOTE 12"]`.
    #[serde(default)]
    pub devices: Vec<String>,
    /// Nested DA binary blob info.
    #[serde(default)]
    pub da: Option<FileBlob>,
    /// Optional companion auth file (e.g. for SLA/DAA secured chipsets).
    #[serde(default)]
    pub auth: Option<FileBlob>,
    /// Raw download URL for the blob (flat fallback).
    #[serde(default)]
    pub url: String,
    /// SHA-256 of the blob (flat fallback).
    #[serde(default)]
    pub sha256: String,
    /// Optional raw download URL for auth file (flat fallback).
    #[serde(default)]
    pub auth_url: Option<String>,
    /// Optional SHA-256 for auth file (flat fallback).
    #[serde(default)]
    pub auth_sha256: Option<String>,
    /// Whether verified working by community.
    #[serde(default)]
    pub verified: Option<bool>,
    /// Optional notes / instructions.
    #[serde(default)]
    pub notes: Option<String>,
}

impl DAEntry {
    /// Resolved DA download URL.
    #[must_use]
    pub fn da_url(&self) -> &str {
        if let Some(ref d) = self.da {
            &d.url
        } else {
            &self.url
        }
    }

    /// Resolved DA SHA-256 digest.
    #[must_use]
    pub fn da_sha256(&self) -> &str {
        if let Some(ref d) = self.da {
            &d.sha256
        } else {
            &self.sha256
        }
    }

    /// Resolved companion Auth download URL if present.
    #[must_use]
    pub fn auth_url(&self) -> Option<&str> {
        if let Some(ref a) = self.auth {
            Some(&a.url)
        } else {
            self.auth_url.as_deref()
        }
    }

    /// Resolved companion Auth SHA-256 digest if present.
    #[must_use]
    pub fn auth_sha256(&self) -> Option<&str> {
        if let Some(ref a) = self.auth {
            Some(&a.sha256)
        } else {
            self.auth_sha256.as_deref()
        }
    }

    /// Returns `true` if this DA has a companion Auth file.
    #[must_use]
    pub const fn has_auth(&self) -> bool {
        self.auth.is_some() || self.auth_url.is_some()
    }

    /// Human-readable label for pickers: `"Infinix NOTE 12  (infinix · mt6789)"`.
    #[must_use]
    pub fn label(&self) -> String {
        let device = self.devices.first().map_or("unknown device", String::as_str);
        format!("{device}  ({brand} · {chipset})", brand = self.brand, chipset = self.chipset)
    }
}

/// The DA manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DAManifest {
    pub version: String,
    #[serde(default)]
    pub dais: Vec<DAEntry>,
}

/// Fetch and parse the DA manifest.
///
/// # Errors
///
/// Returns [`PenumbraError::ManifestFetch`] on network failure or malformed JSON.
pub fn fetch_da_manifest() -> Result<DAManifest> {
    let mut res = ureq::get(DA_MANIFEST_URL)
        .call()
        .map_err(|source| PenumbraError::ManifestFetch(source.to_string()))?;
    let body = res
        .body_mut()
        .read_to_string()
        .map_err(|source| PenumbraError::ManifestFetch(source.to_string()))?;
    serde_json::from_str(&body).map_err(|source| PenumbraError::ManifestFetch(source.to_string()))
}

/// Fetch the manifest and return its entries.
///
/// # Errors
///
/// Returns [`PenumbraError::ManifestFetch`] on network failure or malformed JSON.
pub fn list_dais() -> Result<Vec<DAEntry>> {
    Ok(fetch_da_manifest()?.dais)
}

/// Normalized lowercase form used for fuzzy matching.
fn normalize(s: &str) -> String {
    s.to_lowercase()
}

/// Resolve a DA by retail device name.
///
/// Matching is case-insensitive substring scoring over `devices` (primary)
/// and `(brand, chipset)` (fallback). Returns a hint listing close matches on
/// a no-hit.
///
/// # Errors
///
/// Returns [`PenumbraError::NoSuchDa`] (with close-match hints) when nothing
/// matches.
pub fn resolve_by_device(query: &str) -> Result<DAEntry> {
    let dais = list_dais()?;
    resolve_by_device_in(&dais, query)
}

/// [`resolve_by_device`] over an explicit entry list (testable, no network).
pub(crate) fn resolve_by_device_in(dais: &[DAEntry], query: &str) -> Result<DAEntry> {
    let q = normalize(query);
    let q_tokens: Vec<&str> = q.split_whitespace().collect();

    let mut best: Option<(usize, &DAEntry)> = None;
    for entry in dais {
        let mut score = 0;
        let mut candidates: Vec<&str> =
            entry.devices.iter().map(String::as_str).collect();
        candidates.extend([entry.brand.as_str(), entry.chipset.as_str()]);
        for name in candidates {
            let n = normalize(name);
            if n == q {
                score = 1000;
            } else if n.contains(&q) {
                score = score.max(500);
            } else if q_tokens.iter().all(|t| n.contains(t)) {
                score = score.max(300);
            }
        }
        if let Some((best_score, _)) = best {
            if score > best_score {
                best = Some((score, entry));
            }
        } else if score > 0 {
            best = Some((score, entry));
        }
    }

    if let Some((_, entry)) = best {
        return Ok(entry.clone());
    }

    let hints: Vec<String> = dais
        .iter()
        .flat_map(|e| e.devices.iter().map(Clone::clone))
        .take(5)
        .collect();
    let hint_msg = if hints.is_empty() {
        String::new()
    } else {
        format!(" (did you mean: {})", hints.join(", "))
    };
    Err(PenumbraError::NoSuchDa { query: format!("{query}{hint_msg}") })
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

/// [`resolve_by_brand_chipset`] over an explicit entry list (testable, no network).
pub(crate) fn resolve_by_brand_chipset_in(
    dais: &[DAEntry],
    brand: &str,
    chipset: &str,
) -> Result<DAEntry> {
    dais.iter()
        .find(|e| {
            normalize(&e.brand) == normalize(brand) && normalize(&e.chipset) == normalize(chipset)
        })
        .cloned()
        .ok_or_else(|| PenumbraError::NoSuchDa {
            query: format!("{brand}/{chipset}"),
        })
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
}
