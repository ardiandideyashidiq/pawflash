//! High-level DA read/write/erase operations.

use crate::mtk::{MtkError, MtkEvent, PartType, Result};
use std::path::Path;

/// Read a partition to `file`; returns bytes read.
///
/// # Errors
///
/// Returns any [`MtkError`] from install or the bridge run.
pub fn read_partition(
    _manifest: &crate::mtk::Manifest,
    _partition: &str,
    _file: &Path,
    _parttype: PartType,
    _on_event: &mut dyn FnMut(&MtkEvent),
) -> Result<u64> {
    Err(MtkError::Bridge("not yet implemented".into()))
}

/// Write `file` to a partition; returns bytes written.
///
/// # Errors
///
/// Returns any [`MtkError`] from install or the bridge run.
pub fn write_partition(
    _manifest: &crate::mtk::Manifest,
    _partition: &str,
    _file: &Path,
    _parttype: PartType,
    _on_event: &mut dyn FnMut(&MtkEvent),
) -> Result<u64> {
    Err(MtkError::Bridge("not yet implemented".into()))
}

/// Erase a partition.
///
/// # Errors
///
/// Returns any [`MtkError`] from install or the bridge run.
pub fn erase_partition(
    _manifest: &crate::mtk::Manifest,
    _partition: &str,
    _parttype: PartType,
    _on_event: &mut dyn FnMut(&MtkEvent),
) -> Result<()> {
    Err(MtkError::Bridge("not yet implemented".into()))
}
