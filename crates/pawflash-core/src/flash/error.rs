use std::path::PathBuf;
use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
pub enum FlashError {
    #[error("no fastboot device found")]
    #[diagnostic(help("connect your device via USB and check that it is in fastboot mode"))]
    NoDevice,

    #[error("device is in ADB mode, not fastboot{}", adb_suffix(.serials))]
    #[diagnostic(help("reboot the device to the bootloader (adb reboot bootloader) to enter fastboot mode"))]
    DeviceInAdb { serials: Vec<String> },

    #[error("USB devices found but none expose a fastboot or ADB interface ({})", vids.join(", "))]
    #[diagnostic(help("connect your device via USB and check that it is in fastboot mode"))]
    NoUsbInterface { vids: Vec<String> },

    #[error("fastboot device detected (VID:PID {vidpid}) but its USB driver is not supported: {}", .driver.as_deref().unwrap_or("unknown"))]
    #[cfg_attr(
        target_os = "windows",
        diagnostic(help("install the Google 'Android Bootloader' / WinUSB driver for the device using Zadig (https://zadig.akeo.ie) or Device Manager"))
    )]
    #[cfg_attr(
        not(target_os = "windows"),
        diagnostic(help("ensure a functional USB driver is associated with the device serial"))
    )]
    WindowsDriver {
        vidpid: String,
        driver: Option<String>,
        serial: Option<String>,
    },

    #[error("device mismatch: expected {expected}, got {actual}")]
    #[diagnostic(help("use --serial SERIAL to target the correct device"))]
    DeviceMismatch { expected: String, actual: String },

    #[error("multiple fastboot devices found ({serials:?}); refusing to guess which to target")]
    #[diagnostic(help("disconnect extra devices or use --serial SERIAL to target the correct device"))]
    MultipleDevices { serials: Vec<String> },

    #[error("fastboot protocol: {0}")]
    Protocol(#[from] fastboot_protocol::nusb::NusbFastBootError),

    #[error("failed to open fastboot device: {0}")]
    Open(#[from] fastboot_protocol::nusb::NusbFastBootOpenError),

    #[error("image not found: {0}")]
    #[diagnostic(help("verify the image path and --firmware-dir"))]
    ImageNotFound(PathBuf),

    #[error("image {name} too large ({image_size}) > partition size ({partition_size})")]
    #[diagnostic(severity(Warning))]
    ImageTooLarge { name: String, image_size: u64, partition_size: i64 },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("download error: {0}")]
    Download(#[from] fastboot_protocol::nusb::DownloadError),

    #[error("flash action failed: {partition}: {reason}")]
    ActionFailed { partition: String, reason: String },

    #[error("failed to parse sparse image header")]
    SparseParseFailed,

    #[error("failed to split sparse image for download")]
    SparseSplitFailed,

    #[error("sparse image truncated: read {read} of {expected} bytes")]
    SparseTruncated { read: usize, expected: usize },

    #[error("flash transfer timed out: {partition}: {step}")]
    #[diagnostic(help("check the USB connection; the device may have stopped responding"))]
    Timeout { partition: String, step: String },

    #[error("flash cancelled by user")]
    Cancelled,
}

impl serde::Serialize for FlashError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.collect_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, FlashError>;

/// Render a trailing serial-list suffix for the ADB-mode error message.
#[must_use]
fn adb_suffix(serials: &[String]) -> String {
    if serials.is_empty() {
        return String::new();
    }
    format!(" (device serial(s): {})", serials.join(", "))
}
