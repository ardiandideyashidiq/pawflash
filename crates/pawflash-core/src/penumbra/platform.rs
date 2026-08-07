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

/// Subdirectory under the base data dir shared by all pawflash submodules.
const APP_SUBDIR: &str = "pawflash";

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
    #[cfg(target_os = "linux")]
    {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map_or_else(
                || {
                    std::env::var_os("HOME")
                        .map(PathBuf::from)
                        .unwrap_or_default()
                        .join(".local/share")
                },
                PathBuf::from,
            );
        base.join(APP_SUBDIR)
    }
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        base.join(APP_SUBDIR)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        std::env::temp_dir().join(APP_SUBDIR)
    }
}

/// The penumbra data directory (DA cache, state, etc.).
#[must_use]
pub fn penumbra_dir() -> PathBuf {
    base_data_dir().join("penumbra")
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
