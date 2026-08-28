//! USBDK prerequisite check and install (Windows).
//!
//! On Linux, all methods are no-ops. On Windows, `ensure_usbdk()` detects
//! the USBDK driver/service, attempts a silent install of the pinned MSI,
//! and falls back to a download URL the user can open in a browser.

use crate::mtk::Result;

/// Pinned USBDK release and MSI asset (verified via GitHub release digest).
pub const USBDK_RELEASE_TAG: &str = "v1.00-22";
pub const USBDK_MSI_URL: &str =
    "https://github.com/daynix/UsbDk/releases/download/v1.00-22/UsbDk_1.0.22_x64.msi";

/// Whether the USBDK driver/service is installed.
#[must_use]
pub fn usbdk_installed() -> bool {
    crate::platform::CURRENT.usbdk_installed()
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
    crate::platform::CURRENT
        .ensure_driver()
        .map_err(crate::mtk::MtkError::Prerequisite)
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
