//! Platform data-directory resolution.
//!
//! Shared by the `mtk` (bridge install) and `penumbra` (DA cache) modules.
//! The base directory is resolved once here; consumers append their own
//! subdirectory (e.g. `mtk-bridge`, `penumbra`).
//!
//! Priority:
//! 1. `PAWFLASH_DATA_DIR` env var (explicit override, used by tests).
//! 2. Linux: `$XDG_DATA_HOME` or `~/.local/share`, then `pawflash`.
//! 3. Windows: `%LOCALAPPDATA%`, then `pawflash`.
//! 4. Anything else: temp dir, then `pawflash`.

use std::path::PathBuf;

/// The pawflash data directory (without the module-specific subdirectory).
///
/// Honors `PAWFLASH_DATA_DIR` first, then the platform data dir.
#[must_use]
pub fn base_data_dir() -> PathBuf {
    base_data_dir_with(std::env::var_os("PAWFLASH_DATA_DIR").as_deref())
}

/// Testable core of [`base_data_dir`] with an explicit override.
#[must_use]
pub(crate) fn base_data_dir_with(override_dir: Option<&std::ffi::OsStr>) -> PathBuf {
    if let Some(dir) = override_dir {
        return PathBuf::from(dir);
    }
    crate::platform::CURRENT.base_data_dir()
}

/// The penumbra data directory (DA cache, state, etc.).
///
/// Ensures the directory exists before returning the path.
#[must_use]
pub fn penumbra_dir() -> PathBuf {
    let dir = base_data_dir().join("penumbra");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_data_dir_honors_override() {
        assert_eq!(
            base_data_dir_with(Some(std::ffi::OsStr::new("/tmp/pawflash-test"))),
            PathBuf::from("/tmp/pawflash-test")
        );
    }

    #[test]
    fn penumbra_dir_is_under_base() {
        assert_eq!(
            base_data_dir_with(Some(std::ffi::OsStr::new("/tmp/pawflash-test"))).join("penumbra"),
            PathBuf::from("/tmp/pawflash-test/penumbra")
        );
    }
}
