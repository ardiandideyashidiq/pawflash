use std::path::PathBuf;
use std::process::Command;

use super::Platform;

pub(crate) struct Impl;

impl Platform for Impl {
    fn is_candidate_serial_port(&self, name: &str) -> bool {
        name.starts_with("/dev/ttyACM") || name.starts_with("/dev/ttyUSB")
    }

    fn base_data_dir(&self) -> PathBuf {
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
        base.join("pawflash")
    }

    fn bridge_binary_name(&self) -> &'static str {
        "bridge"
    }

    fn manifest_platform_key(&self) -> Result<&'static str, String> {
        if cfg!(target_arch = "x86_64") {
            Ok("linux-x86_64")
        } else {
            Err(format!("unsupported architecture: {}", std::env::consts::ARCH))
        }
    }

    fn ensure_driver(&self) -> Result<(), String> {
        // Linux has no driver prerequisites — usbfs is kernel-native.
        Ok(())
    }

    fn usbdk_installed(&self) -> bool {
        false
    }

    fn permission_error_patterns(&self) -> &'static [&'static str] {
        &["permission denied"]
    }

    fn accept_unknown_port_type(&self) -> bool {
        false
    }

    fn install_udev_rules(&self) -> bool {
        const RULE_PATH: &str = "/etc/udev/rules.d/99-mediatek-preloader.rules";
        const MEDIATEK_UDEV_RULES: &str = r#"# MediaTek Preloader / BROM / Download Agent
# IDs: 0e8d:2000 preloader, 0e8d:0003 DA/BROM (same idVendor, covered below)

SUBSYSTEM=="usb", ATTR{idVendor}=="0e8d", MODE="0666", TAG+="uaccess"
SUBSYSTEM=="tty", ATTRS{idVendor}=="0e8d", MODE="0666", TAG+="uaccess"
"#;

        let existing = std::fs::read_to_string(RULE_PATH).ok();
        if existing.as_deref() == Some(MEDIATEK_UDEV_RULES) {
            return true;
        }

        tracing::warn!("Normal user cannot open preloader serial port. Installing udev rules with sudo.");

        let written = Command::new("sudo")
            .args(["tee", RULE_PATH])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(mut stdin) = child.stdin.take() {
                    stdin.write_all(MEDIATEK_UDEV_RULES.as_bytes())?;
                }
                child.wait()
            })
            .is_ok_and(|status| status.success());

        if !written {
            tracing::warn!("Failed to write udev rules to {RULE_PATH}");
            return false;
        }

        let _ = Command::new("sudo")
            .args(["udevadm", "control", "--reload-rules"])
            .status();
        let _ = Command::new("sudo")
            .args(["udevadm", "trigger"])
            .status();
        let _ = Command::new("sudo")
            .args(["udevadm", "settle"])
            .status();

        tracing::warn!("udev rules installed. Reconnect the device if the port still has old permissions.");
        true
    }

    fn add_user_to_group(&self) -> bool {
        let Ok(user) = std::env::var("USER") else { return false };

        for group in &["dialout", "plugdev"] {
            let already_in = Command::new("id")
                .arg("-nG")
                .arg(&user)
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .is_some_and(|s| s.split_whitespace().any(|g| g == *group));

            if already_in {
                continue;
            }

            if Command::new("sudo")
                .args(["usermod", "-aG", group, &user])
                .status()
                .is_ok_and(|s| s.success())
            {
                tracing::warn!(
                    "Added user '{user}' to group '{group}'. Log out and back in for this to take effect."
                );
                return true;
            }
        }

        false
    }

    fn print_manual_guidance(&self) {
        const RULE_PATH: &str = "/etc/udev/rules.d/99-mediatek-preloader.rules";
        const MEDIATEK_UDEV_RULES: &str = r#"# MediaTek Preloader / BROM / Download Agent
# IDs: 0e8d:2000 preloader, 0e8d:0003 DA/BROM (same idVendor, covered below)

SUBSYSTEM=="usb", ATTR{idVendor}=="0e8d", MODE="0666", TAG+="uaccess"
SUBSYSTEM=="tty", ATTRS{idVendor}=="0e8d", MODE="0666", TAG+="uaccess"
"#;

        let user = std::env::var("USER").unwrap_or_else(|_| "<your-username>".into());
        tracing::warn!(
            "Permission denied opening serial port.\n\
             Install udev rules manually:\n\
             sudo tee {RULE_PATH} >/dev/null <<'EOF'\n{MEDIATEK_UDEV_RULES}EOF\n\
             sudo udevadm control --reload-rules\n\
             sudo udevadm trigger\n\n\
             Or add yourself to the dialout group:\n\
             sudo usermod -a -G dialout {user}\n\
             Then log out and back in."
        );
    }

    fn post_handshake_hint(&self) -> &'static str {
        ""
    }

    fn fastboot_interface_openable(&self, info: &fastboot_protocol::nusb::DeviceInfo) -> bool {
        let _ = info;
        true
    }

    fn fastboot_driver_name(
        &self,
        info: &fastboot_protocol::nusb::DeviceInfo,
    ) -> Option<String> {
        let _ = info;
        None
    }
}
