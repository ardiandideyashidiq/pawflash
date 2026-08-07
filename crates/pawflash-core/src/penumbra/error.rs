//! Error types for the `penumbra` module.
//!
//! Every failure path in the native DA-mode integration maps to a variant
//! here, with actionable help text where the fix is non-obvious (wrong DA,
//! manifest unreachable, hash mismatch, device busy).

use miette::Diagnostic;
use thiserror::Error;

/// Errors raised by the penumbra DA integration.
#[derive(Error, Debug, Diagnostic)]
pub enum PenumbraError {
    /// The DA manifest could not be fetched or parsed.
    #[error("failed to fetch DA manifest: {0}")]
    #[diagnostic(help("check connectivity; the manifest lives on the penumbra fork"))]
    ManifestFetch(String),

    /// A DA entry could not be resolved from the manifest.
    #[error("no DA found for {query}")]
    #[diagnostic(help("run `pawflash penumbra da download` to browse supported devices"))]
    NoSuchDa { query: String },

    /// The DA file failed to download.
    #[error("failed to download {url}: {source}")]
    Download {
        url: String,
        #[source]
        source: ureq::Error,
    },

    /// The downloaded DA file failed its SHA-256 check.
    #[error("DA file failed sha256 verification (expected {expected}, got {actual})")]
    #[diagnostic(help("download may be corrupted — retry with `pawflash penumbra da download`"))]
    HashMismatch { expected: String, actual: String },

    /// Local DA cache/state bookkeeping failed.
    #[error("DA cache failed: {0}")]
    Cache(String),

    /// No DA is selected for an operation.
    #[error("no DA selected for this operation")]
    #[diagnostic(help("run `pawflash penumbra da download` first, or pass `--da <path>`"))]
    NoDaSelected,

    /// The penumbra library reported an error.
    #[error("{0}")]
    Penumbra(String),

    /// Another pawflash process holds the device lock.
    #[error("another pawflash process is using the device")]
    #[diagnostic(help("wait for it to finish, or kill the stale process and retry"))]
    DeviceBusy,

    /// The device lock file could not be created or locked.
    #[error("failed to acquire device lock: {source}")]
    DeviceLock {
        #[source]
        source: std::io::Error,
    },

    /// No MTK device appeared within the wait window.
    #[error("no MTK device found (BROM/preloader/DA) within {wait:?}")]
    #[diagnostic(help("plug the device in and retry; on Linux check udev rules"))]
    NoDevice { wait: std::time::Duration },

    /// The DA does not support the connected SoC.
    #[error("DA is not compatible with this device (hardware code 0x{hw_code:04X})")]
    #[diagnostic(help("run `pawflash penumbra da download` and pick a DA for your device"))]
    DaMismatch { hw_code: u16 },

    /// A prerequisite (udev rules / WinUSB driver) is not satisfied.
    #[error("device prerequisite not satisfied: {0}")]
    Prerequisite(String),
}

pub type Result<T> = std::result::Result<T, PenumbraError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_messages_are_actionable() {
        assert!(PenumbraError::NoDaSelected.to_string().contains("no DA selected"));
        assert!(PenumbraError::NoSuchDa { query: "note 12".into() }
            .to_string()
            .contains("note 12"));
        assert!(PenumbraError::DaMismatch { hw_code: 0x6789 }.to_string().contains("0x6789"));
    }
}
