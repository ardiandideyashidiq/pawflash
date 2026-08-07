//! Persisted last-used DA selection.
//!
//! Ops need a DA at `DeviceBuilder` build time. To avoid asking the user every
//! time, the last `da download` selection is persisted to
//! `penumbra_dir()/state.json` and reloaded by op resolution (unless `--da`
//! overrides it).

use crate::penumbra::{PenumbraError, Result, penumbra_dir};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// The state file path.
fn state_path() -> std::path::PathBuf {
    penumbra_dir().join("state.json")
}

/// The last-used DA selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaSelection {
    pub brand: String,
    pub chipset: String,
    pub path: String,
    pub sha256: String,
}

/// Load the persisted selection, if any.
#[must_use]
pub fn load_selection() -> Option<DaSelection> {
    load_selection_at(&state_path())
}

/// Testable core of [`load_selection`]; `None` when absent or malformed.
fn load_selection_at(path: &Path) -> Option<DaSelection> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Persist `sel` to the state file.
///
/// # Errors
///
/// Returns [`PenumbraError::Cache`] on write failure.
pub fn save_selection(sel: &DaSelection) -> Result<()> {
    save_selection_at(&state_path(), sel)
}

/// Testable core of [`save_selection`] with an explicit path.
///
/// # Errors
///
/// Returns [`PenumbraError::Cache`] on write failure.
fn save_selection_at(path: &Path, sel: &DaSelection) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| PenumbraError::Cache("state file has no parent".into()))?;
    std::fs::create_dir_all(parent)
        .map_err(|source| PenumbraError::Cache(source.to_string()))?;
    let json = serde_json::to_string_pretty(sel)
        .map_err(|source| PenumbraError::Cache(source.to_string()))?;
    std::fs::write(path, json).map_err(|source| PenumbraError::Cache(source.to_string()))
}

/// Clear the persisted selection (and remove the state file).
///
/// # Errors
///
/// Returns [`PenumbraError::Cache`] on filesystem failure.
pub fn clear_selection() -> Result<()> {
    let path = state_path();
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|source| PenumbraError::Cache(source.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sel() -> DaSelection {
        DaSelection {
            brand: "infinix".into(),
            chipset: "mt6789".into(),
            path: "/tmp/da/infinix-mt6789.bin".into(),
            sha256: "3c7de4ee52b47f1d4c5122868b52dfa06c18e5ef940f4c8a04c46365a696bbdd".into(),
        }
    }

    #[test]
    fn save_then_load_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("sub").join("state.json");
        assert!(load_selection_at(&path).is_none());
        save_selection_at(&path, &sel()).unwrap();
        assert_eq!(load_selection_at(&path), Some(sel()));
    }

    #[test]
    fn load_missing_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load_selection_at(&tmp.path().join("nope.json")).is_none());
    }

    #[test]
    fn load_malformed_is_none() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("state.json");
        std::fs::write(&path, "not json").unwrap();
        assert!(load_selection_at(&path).is_none());
    }

    #[test]
    fn save_overwrites_previous() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("state.json");
        let mut other = sel();
        other.chipset = "mt6768".into();
        save_selection_at(&path, &sel()).unwrap();
        save_selection_at(&path, &other).unwrap();
        assert_eq!(load_selection_at(&path), Some(other));
    }
}
