//! Error types for the `mtk` bridge module.
//!
//! Every failure path in the DA-mode integration maps to a variant here, with
//! actionable help text where the fix is non-obvious (hash mismatch, missing
//! manifest, unsupported platform).

use miette::Diagnostic;
use thiserror::Error;

/// Errors raised by the mtkclient bridge integration.
#[derive(Error, Debug, Diagnostic)]
pub enum MtkError {
    /// The release manifest could not be fetched or parsed.
    #[error("failed to fetch mtk bridge manifest: {0}")]
    ManifestFetch(String),

    /// The requested platform is not supported by the release.
    #[error("unsupported platform: {0}")]
    #[diagnostic(help("mtk bridge releases only provide linux-x86_64 and windows-x86_64"))]
    UnsupportedPlatform(String),

    /// The requested platform asset is missing from the manifest.
    #[error("no {platform} asset in mtk bridge manifest")]
    #[diagnostic(help("the release may not have been built for this platform"))]
    MissingAsset { platform: String },

    /// The bridge archive failed to download.
    #[error("failed to download {url}: {source}")]
    Download {
        url: String,
        #[source]
        source: ureq::Error,
    },

    /// The downloaded archive failed its SHA-256 check.
    #[error("mtk bridge archive failed sha256 verification (expected {expected}, got {actual})")]
    #[diagnostic(help("download may be corrupted — retry with `pawflash mtkclient download`"))]
    HashMismatch { expected: String, actual: String },

    /// The archive could not be unpacked.
    #[error("failed to extract mtk bridge archive: {0}")]
    Extract(String),

    /// Local install/version bookkeeping failed.
    #[error("mtk bridge install failed: {0}")]
    Install(String),

    /// The bridge process could not be spawned.
    #[error("failed to spawn mtk bridge `{bin}`: {source}")]
    #[diagnostic(help("try `pawflash mtkclient doctor` to verify the installation"))]
    Spawn {
        bin: String,
        #[source]
        source: std::io::Error,
    },

    /// The bridge process timed out.
    #[error("mtk bridge timed out")]
    #[diagnostic(help("the device may not be responding in download agent mode"))]
    Timeout,

    /// The bridge emitted an error event.
    #[error("mtk bridge error: {0}")]
    Bridge(String),

    /// The bridge protocol stream was malformed.
    #[error("mtk bridge protocol error: {0}")]
    Protocol(String),

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

    /// A prerequisite (udev rules / USBDK) is not satisfied.
    #[error("device prerequisite not satisfied: {0}")]
    Prerequisite(String),
}

pub type Result<T> = std::result::Result<T, MtkError>;
