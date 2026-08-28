//! USB device detection and udev rule management for mtkclient DA devices.
//!
//! Device detection works on both Linux and Windows via `nusb`.

/// USB vendor IDs that mtkclient treats as DA-capable devices
/// (`mtkclient/config/usb_ids.py`): `MediaTek`, LG, OPPO, Sony.
pub const DEVICE_VENDOR_IDS: [u16; 4] = [0x0e8d, 0x1004, 0x22d9, 0x0fce];

/// Return the udev rules content for mtkclient DA devices.
#[cfg(target_os = "linux")]
#[must_use]
pub const fn rules_content() -> &'static str {
    RULES
}

#[cfg(target_os = "linux")]
const RULES: &str = r#"# MediaTek / LG / OPPO / Sony Download Agent devices (mtkclient)
# USB access for the frozen mtkclient bridge; tty only for the 0e8d preloader.

SUBSYSTEM=="usb", ATTR{idVendor}=="0e8d", MODE="0666", TAG+="uaccess"
SUBSYSTEM=="usb", ATTR{idVendor}=="1004", MODE="0666", TAG+="uaccess"
SUBSYSTEM=="usb", ATTR{idVendor}=="22d9", MODE="0666", TAG+="uaccess"
SUBSYSTEM=="usb", ATTR{idVendor}=="0fce", MODE="0666", TAG+="uaccess"
SUBSYSTEM=="tty", ATTRS{idVendor}=="0e8d", MODE="0666", TAG+="uaccess"
"#;

/// Return the udev rules content on non-Linux (empty marker).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub const fn rules_content() -> &'static str {
    ""
}

/// Install the mtkclient udev rules. Delegates to the platform trait.
#[must_use]
pub fn ensure_udev_rules() -> bool {
    crate::platform::CURRENT.install_udev_rules()
}

/// Whether any DA-capable USB device is present.
///
/// Enumerates devices via `nusb::list_devices()` and matches against
/// [`DEVICE_VENDOR_IDS`].
pub async fn device_visible() -> bool {
    match nusb::list_devices().await {
        Ok(mut devices) => devices.any(|d| DEVICE_VENDOR_IDS.contains(&d.vendor_id())),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn rules_content_contains_expected_vids() {
        let content = rules_content();
        assert!(content.contains("0e8d"));
        assert!(content.contains("1004"));
        assert!(content.contains("22d9"));
        assert!(content.contains("0fce"));
        assert!(content.contains("MODE=\"0666\""));
        // tty only for the preloader vendor.
        assert!(content.contains("SUBSYSTEM==\"tty\", ATTRS{idVendor}==\"0e8d\""));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rules_content_is_a_marker_worthy_static() {
        // The rules content doubles as the up-to-date marker: two calls must
        // return byte-identical strings so the idempotence check is exact.
        assert_eq!(rules_content(), rules_content());
    }

    #[test]
    fn vendor_ids_match_mtkclient() {
        assert_eq!(DEVICE_VENDOR_IDS, [0x0e8d, 0x1004, 0x22d9, 0x0fce]);
    }
}
