//! Download, verify, and atomic install of the frozen mtk bridge.

use crate::mtk::{MtkError, Result};
use std::path::PathBuf;

/// Platform data directory holding the installed bridge.
#[must_use]
pub fn install_root() -> PathBuf {
    PathBuf::from("unimplemented")
}

/// Installed bridge version, if any.
#[must_use]
pub const fn current_version() -> Option<String> {
    None
}

/// Ensure the bridge for `manifest` is installed; return the binary path.
///
/// # Errors
///
/// Returns [`MtkError::Install`] on any download/verify/extract failure.
pub fn ensure_installed(_manifest: &crate::mtk::Manifest) -> Result<PathBuf> {
    Err(MtkError::Install("not yet implemented".into()))
}
