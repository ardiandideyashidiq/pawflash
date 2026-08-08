//! USB device detection and (on Linux) udev rule management for mtkclient DA
//! devices.
//!
//! Udev installation is gated behind `target_os = "linux"`; on other platforms
//! the rule entry points become no-ops so the rest of the codebase needs no
//! `cfg`. Device detection works on both Linux and Windows via `nusb`.

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
const RULE_PATH: &str = "/etc/udev/rules.d/99-mediatek-mtkclient.rules";

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

/// Install the mtkclient udev rules. Returns `true` if the rules are present
/// (either already up to date, or freshly written).
///
/// Uses `sudo tee` with piped stdin when run on a TTY, `pkexec tee` otherwise.
#[cfg(target_os = "linux")]
#[must_use]
pub fn ensure_udev_rules() -> bool {
    let existing = std::fs::read_to_string(RULE_PATH).ok();
    if existing.as_deref() == Some(RULES) {
        return true;
    }

    let elevated = if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        "sudo"
    } else {
        "pkexec"
    };
    let written = std::process::Command::new(elevated)
        .args(["tee", RULE_PATH])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(RULES.as_bytes())?;
            }
            child.wait()
        })
        .is_ok_and(|status| status.success());

    if !written {
        tracing::warn!("failed to write udev rules to {RULE_PATH}");
        return false;
    }

    for args in [
        ["udevadm", "control", "--reload-rules"],
        ["udevadm", "trigger", ""],
        ["udevadm", "settle", ""],
    ] {
        let filtered: Vec<&str> = args.into_iter().filter(|a| !a.is_empty()).collect();
        let _ = std::process::Command::new(elevated).args(&filtered).status();
    }

    tracing::warn!("mtkclient udev rules installed. Reconnect the device if permissions are stale.");
    true
}

/// Non-Linux no-op: returns `true` (nothing to install).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn ensure_udev_rules() -> bool {
    true
}

/// Whether any DA-capable USB device is present.
///
/// Enumerates devices via `nusb::list_devices()` and matches against
/// [`DEVICE_VENDOR_IDS`]. Works on Linux (usbfs) and Windows (`WinUSB`).
pub async fn device_visible() -> bool {
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    {
        match nusb::list_devices().await {
            Ok(mut devices) => devices.any(|d| DEVICE_VENDOR_IDS.contains(&d.vendor_id())),
            Err(_) => false,
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = DEVICE_VENDOR_IDS;
        false
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

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
