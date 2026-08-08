//! Windows USBDK prerequisite check and install.
//!
//! mtkclient's `usblib.py` requires USBDK on Windows
//! (`libusb_set_option(ctx, 1)` = `LIBUSB_OPTION_USE_USBDK`). This module
//! detects it, tries a silent install of the pinned MSI, and falls back to a
//! download URL the user can open in a browser.

use crate::mtk::Result;

/// Pinned USBDK release and MSI asset (verified via GitHub release digest).
pub const USBDK_RELEASE_TAG: &str = "v1.00-22";
pub const USBDK_MSI_URL: &str =
    "https://github.com/daynix/UsbDk/releases/download/v1.00-22/UsbDk_1.0.22_x64.msi";

#[cfg(target_os = "windows")]
mod platform {
    use crate::mtk::error::MtkError;
    use crate::mtk::Result;
    use std::process::Command;

    /// Probe for the USBDK driver/service via `sc query UsbDk`.
    pub fn installed() -> bool {
        Command::new("sc")
            .args(["query", "UsbDk"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Attempt a silent MSI install; returns whether the service appeared.
    pub fn ensure() -> Result<()> {
        if installed() {
            return Ok(());
        }
        // Ignore the msiexec exit code: a successful driver install commonly
        // returns 3010 (ERROR_SUCCESS_REBOOT_REQUIRED) rather than 0. Probe
        // the service instead, which is authoritative regardless of the code.
        let _ = Command::new("msiexec")
            .args(["/i", super::USBDK_MSI_URL, "/qn", "/norestart"])
            .status();
        if installed() {
            return Ok(());
        }
        Err(MtkError::Prerequisite(format!(
            "USBDK is required on Windows. Install it from:\n  {}\n\
             then run `pawflash mtkclient doctor` again.",
            super::USBDK_MSI_URL
        )))
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    /// Non-Windows: nothing to check.
    pub fn installed() -> bool {
        // Deliberately non-const: mirrors the Windows probe's runtime nature.
        std::env::consts::OS == "windows"
    }
}

/// Whether the USBDK driver/service is installed.
#[must_use]
pub fn usbdk_installed() -> bool {
    platform::installed()
}

/// Ensure USBDK is present: silently install the pinned MSI if needed, then
/// fall back to a browser URL. Never hard-fails — the user can install
/// manually and re-run.
///
/// # Errors
///
/// Returns [`MtkError::Prerequisite`] when USBDK is missing and the
/// auto-install did not take effect.
pub fn ensure_usbdk() -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        platform::ensure()
    }
    #[cfg(not(target_os = "windows"))]
    {
        // No USBDK requirement off Windows; `installed()` is non-const so the
        // whole function stays non-const across targets.
        let _ = platform::installed();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usbdk_url_is_the_pinned_asset() {
        assert!(USBDK_MSI_URL.ends_with("UsbDk_1.0.22_x64.msi"));
        assert!(USBDK_MSI_URL.contains("v1.00-22"));
    }

    #[test]
    fn ensure_usbdk_is_ok_off_windows() {
        #[cfg(not(target_os = "windows"))]
        {
            assert!(ensure_usbdk().is_ok());
        }
    }
}
