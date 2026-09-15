//! Local caching of remote DA manifest and custom user-added DA definitions.

use crate::penumbra::manifest::types::{DAEntry, DAManifest};
use crate::penumbra::{penumbra_dir, PenumbraError, Result};
use std::path::PathBuf;

/// Path to the cached remote manifest on disk (`penumbra_dir()/manifest.json`).
#[must_use]
pub fn manifest_cache_path() -> PathBuf {
    penumbra_dir().join("manifest.json")
}

/// Path to user-defined custom added DA entries on disk (`penumbra_dir()/custom-da.json`).
#[must_use]
pub fn custom_da_path() -> PathBuf {
    penumbra_dir().join("custom-da.json")
}

/// Load the locally cached DA manifest if present and valid.
#[must_use]
pub fn load_cached_manifest() -> Option<DAManifest> {
    let path = manifest_cache_path();
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Persist the fetched manifest to local cache for offline usage.
///
/// # Errors
///
/// Returns [`PenumbraError::Cache`] on I/O failure.
pub fn save_cached_manifest(manifest: &DAManifest) -> Result<()> {
    let path = manifest_cache_path();
    let parent = path
        .parent()
        .ok_or_else(|| PenumbraError::Cache("no parent dir for manifest cache".into()))?;
    std::fs::create_dir_all(parent).map_err(|e| PenumbraError::Cache(e.to_string()))?;
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|e| PenumbraError::Cache(e.to_string()))?;
    std::fs::write(&path, json).map_err(|e| PenumbraError::Cache(e.to_string()))
}

/// Load user-defined custom DA entries from `custom-da.json` if present.
#[must_use]
pub fn load_custom_dais() -> Vec<DAEntry> {
    let path = custom_da_path();
    let Ok(bytes) = std::fs::read(&path) else {
        return Vec::new();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

/// Add or update a custom DA entry in `custom-da.json`.
///
/// If an entry with the same ID or `(brand, chipset)` already exists, it is replaced;
/// otherwise the new entry is appended.
///
/// # Errors
///
/// Returns [`PenumbraError::Cache`] on I/O failure.
pub fn save_custom_da(entry: &DAEntry) -> Result<()> {
    let path = custom_da_path();
    let mut entries = load_custom_dais();
    let mut custom_entry = entry.clone();
    custom_entry.is_custom = Some(true);

    let key_matches = |e: &DAEntry| -> bool {
        if let (Some(id_a), Some(id_b)) = (&e.id, &custom_entry.id) {
            id_a.eq_ignore_ascii_case(id_b)
        } else {
            e.brand.eq_ignore_ascii_case(&custom_entry.brand)
                && e.chipset.eq_ignore_ascii_case(&custom_entry.chipset)
        }
    };

    if let Some(pos) = entries.iter().position(key_matches) {
        entries[pos] = custom_entry;
    } else {
        entries.push(custom_entry);
    }

    let parent = path
        .parent()
        .ok_or_else(|| PenumbraError::Cache("no parent dir for custom da".into()))?;
    std::fs::create_dir_all(parent).map_err(|e| PenumbraError::Cache(e.to_string()))?;
    let json = serde_json::to_string_pretty(&entries)
        .map_err(|e| PenumbraError::Cache(e.to_string()))?;
    std::fs::write(&path, json).map_err(|e| PenumbraError::Cache(e.to_string()))
}
