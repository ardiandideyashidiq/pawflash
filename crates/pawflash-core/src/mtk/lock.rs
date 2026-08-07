//! Cross-process device contention lock.
//!
//! DA-mode operations must not run concurrently: two processes flashing a
//! device at once corrupt it. A lock file under the install data dir, held via
//! [`fs4::fs_std::FileExt::try_lock`] (advisory, non-blocking), serializes
//! access. The [`DeviceLock`] guard releases the lock on drop.
//!
//! `fs4` is used rather than `std::fs::File::try_lock`, which was only
//! stabilized in Rust 1.89 (workspace MSRV is 1.85).

use crate::mtk::error::MtkError;
use crate::mtk::install::install_root;
use crate::mtk::Result;
use fs4::FileExt;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use tracing::debug;

/// RAII guard for the device lock; releases on drop.
pub struct DeviceLock {
    _file: File,
}

/// Path to the device lock file.
fn lock_path() -> PathBuf {
    install_root().join("device.lock")
}

/// Acquire the lock on `path`.
fn acquire_at(path: &std::path::Path) -> Result<DeviceLock> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|source| MtkError::DeviceLock { source })?;

    match FileExt::try_lock(&file) {        Ok(()) => {
            let mut w = file.try_clone().map_err(|source| MtkError::DeviceLock { source })?;
            let _ = writeln!(w, "{}", std::process::id());
            debug!(lock = %path.display(), "device lock acquired");
            Ok(DeviceLock { _file: file })
        }
        Err(fs4::TryLockError::WouldBlock) => {
            debug!(lock = %path.display(), "device lock busy");
            Err(MtkError::DeviceBusy)
        }
        Err(fs4::TryLockError::Error(source)) => Err(MtkError::DeviceLock { source }),
    }
}

/// Acquire the device contention lock, failing immediately if another process
/// holds it.
///
/// # Errors
///
/// Returns [`MtkError::DeviceBusy`] when another process holds the lock, or
/// [`MtkError::DeviceLock`] when the lock file cannot be created/locked.
pub fn acquire_device_lock() -> Result<DeviceLock> {
    let path = lock_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| MtkError::DeviceLock { source })?;
    }
    acquire_at(&path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_lock_fails_while_first_held() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("device.lock");
        let _first = acquire_at(&path).unwrap();
        let second = acquire_at(&path);
        assert!(matches!(second, Err(MtkError::DeviceBusy)));
    }

    #[test]
    fn lock_reacquires_after_drop() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("device.lock");
        {
            let _first = acquire_at(&path).unwrap();
        }
        let second = acquire_at(&path);
        assert!(second.is_ok());
    }

    #[test]
    fn lock_path_is_under_install_root() {
        assert!(lock_path().to_string_lossy().contains("mtk-bridge"));
    }
}
