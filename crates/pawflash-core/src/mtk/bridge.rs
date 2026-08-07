//! Bridge process runner and JSON-lines protocol parser.

use crate::mtk::{MtkError, MtkEvent, Result};
use std::path::Path;

/// Run one command against the bridge binary.
///
/// # Errors
///
/// Returns [`MtkError::Spawn`], [`MtkError::Protocol`], or [`MtkError::Bridge`]
/// on process/parse/op failures.
pub fn run_bridge(
    _bin: &Path,
    _cmd: &crate::mtk::MtkCommand,
    _on_event: &mut dyn FnMut(&MtkEvent),
) -> Result<crate::mtk::MtkOutcome> {
    Err(MtkError::Bridge("not yet implemented".into()))
}
