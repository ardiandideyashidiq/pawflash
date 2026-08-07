//! Device open + connect-time DA/SoC compatibility.
//!
//! penumbra needs a DA at `DeviceBuilder` build time and refuses to load an
//! incompatible one at `init()`. [`open_device`] acquires the shared device
//! lock, waits for an MTK port, builds a device with the DA, and remaps the
//! "no compatible DA" failure to an actionable hint.

use crate::mtk::lock::{DeviceLock, acquire_device_lock};
use crate::penumbra::{PenumbraError, Result};
use penumbra::{DAFile, Device, DeviceBuilder, find_mtk_port};
use std::path::Path;
use std::time::{Duration, Instant};

/// Default time to wait for an MTK device to appear.
pub const DEFAULT_WAIT: Duration = Duration::from_secs(30);

/// A connected penumbra device holding the device contention lock.
pub struct PenumbraDevice {
    /// The open device.
    pub device: Device,
    /// Cross-process device lock, released on drop.
    _lock: DeviceLock,
}

/// Open an MTK device with `da_bytes`, waiting up to `wait` for a port.
///
/// Acquires the shared device lock first (fails fast if another pawflash
/// process holds it), then polls [`find_mtk_port`] until a device appears or
/// `wait` elapses.
///
/// # Errors
///
/// Returns [`PenumbraError::DeviceBusy`] if the lock is held, [`PenumbraError::NoDevice`]
/// if no port appears, [`PenumbraError::DaMismatch`] if the DA doesn't support
/// the connected `SoC`, or a wrapped penumbra error otherwise.
pub fn open_device(da_bytes: Vec<u8>, wait: Duration) -> Result<PenumbraDevice> {
    let lock = acquire_device_lock().map_err(map_lock_err)?;
    let port = wait_for_port(wait, Duration::from_millis(500))?;
    let da_hint = da_bytes_parse_hint(&da_bytes);

    let mut device = DeviceBuilder::default()
        .with_mtk_port(port)
        .with_da_data(da_bytes)
        .build()
        .map_err(|e| PenumbraError::Penumbra(e.to_string()))?;

    device
        .init()
        .map_err(|e| remap_init_error(&e, da_hint))?;

    Ok(PenumbraDevice { device, _lock: lock })
}

/// Map an mtk lock error to a penumbra one.
fn map_lock_err(e: crate::mtk::MtkError) -> PenumbraError {
    match e {
        crate::mtk::MtkError::DeviceBusy => PenumbraError::DeviceBusy,
        other => PenumbraError::DeviceLock {
            source: std::io::Error::other(other.to_string()),
        },
    }
}

/// Poll [`find_mtk_port`] until a port appears or `wait` elapses.
fn wait_for_port(wait: Duration, tick: Duration) -> Result<Box<dyn penumbra::MTKPort>> {
    let start = Instant::now();
    loop {
        if let Some(port) = find_mtk_port() {
            return Ok(port);
        }
        if start.elapsed() >= wait {
            return Err(PenumbraError::NoDevice { wait });
        }
        std::thread::sleep(tick);
    }
}

/// Try to parse the DA bytes to extract an `hw_code` for the error hint.
fn da_bytes_parse_hint(da_bytes: &[u8]) -> Option<u16> {
    DAFile::parse_da(da_bytes)
        .ok()
        .and_then(|f| f.das.first().map(|d| d.hw_code))
}

/// Remap penumbra's `init` error: an incompatible DA gets an actionable hint.
fn remap_init_error(e: &penumbra::error::Error, da_hint: Option<u16>) -> PenumbraError {
    let msg = e.to_string();
    if msg.contains("No compatible DA") {
        // The hw_code is embedded in the message as 0xXXXX.
        let hw_code = parse_hw_code(&msg).unwrap_or(0);
        return PenumbraError::DaMismatch { hw_code };
    }
    let da_note = match da_hint {
        Some(code) => format!(" (DA advertises hw_code 0x{code:04X})"),
        None => String::new(),
    };
    PenumbraError::Penumbra(format!("{msg}{da_note}"))
}

/// Extract `0xXXXX` from a message like `No compatible DA for hardware code 0x0677`.
fn parse_hw_code(msg: &str) -> Option<u16> {
    msg.split("0x").nth(1).and_then(|s| {
        let hex: String = s.chars().take_while(char::is_ascii_hexdigit).collect();
        u16::from_str_radix(&hex, 16).ok()
    })
}

/// Whether a parsed DA file supports `hw_code` (testable, no device).
#[must_use]
pub fn da_supports_hw_code(da: &DAFile, hw_code: u16) -> bool {
    da.get_da_from_hw_code(hw_code).is_some()
}

/// Whether `da_bytes` parse and support `hw_code`.
#[must_use]
pub fn da_bytes_support_hw_code(da_bytes: &[u8], hw_code: u16) -> bool {
    DAFile::parse_da(da_bytes).is_ok_and(|f| da_supports_hw_code(&f, hw_code))
}

/// Parse a DA file from disk (best-effort; used for status/diagnostic output).
///
/// # Errors
///
/// Returns [`PenumbraError::Cache`] on read/parse failure.
pub fn parse_da_file(path: &Path) -> Result<DAFile> {
    let bytes = std::fs::read(path).map_err(|source| PenumbraError::Cache(source.to_string()))?;
    DAFile::parse_da(&bytes).map_err(|e| PenumbraError::Cache(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hw_code_extracts_hex() {
        assert_eq!(parse_hw_code("No compatible DA for hardware code 0x0677"), Some(0x0677));
        assert_eq!(parse_hw_code("No compatible DA for hardware code 0x6789"), Some(0x6789));
        assert_eq!(parse_hw_code("no hex here"), None);
    }

    #[test]
    fn remap_init_error_da_mismatch() {
        let e = penumbra::error::Error::penumbra("No compatible DA for hardware code 0x0677");
        let mapped = remap_init_error(&e, None);
        assert!(matches!(mapped, PenumbraError::DaMismatch { hw_code: 0x0677 }));
    }

    #[test]
    fn remap_init_error_other_keeps_message() {
        let e = penumbra::error::Error::penumbra("boom");
        let mapped = remap_init_error(&e, None);
        assert!(matches!(mapped, PenumbraError::Penumbra(ref m) if m.contains("boom")));
    }

    #[test]
    fn remap_init_error_appends_da_hint() {
        let e = penumbra::error::Error::penumbra("connection reset");
        let mapped = remap_init_error(&e, Some(0x6789));
        assert!(matches!(mapped, PenumbraError::Penumbra(ref m) if m.contains("0x6789")));
    }

    #[test]
    fn wait_for_port_expires_without_device() {
        // No MTK device in this test environment; a short wait should time out.
        let err = wait_for_port(Duration::from_millis(50), Duration::from_millis(10)).unwrap_err();
        assert!(matches!(err, PenumbraError::NoDevice { .. }));
    }

    #[test]
    fn da_supports_hw_code_missing_returns_false() {
        // Empty/nonexistent DA bytes should parse to something or fail; either
        // way a garbage blob cannot support an arbitrary hw_code.
        let da = DAFile::parse_da(&[0u8; 0x6C + 0xDC]).ok();
        let supported = da.as_ref().is_some_and(|f| da_supports_hw_code(f, 0x6789));
        assert!(!supported);
    }
}
