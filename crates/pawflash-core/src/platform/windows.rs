#[cfg(target_os = "windows")]
use std::io::Read;
#[cfg(target_os = "windows")]
use std::path::PathBuf;

#[cfg(target_os = "windows")]
use super::Platform;

#[cfg(target_os = "windows")]
pub(crate) struct Impl;

#[cfg(target_os = "windows")]
impl Platform for Impl {
    fn is_candidate_serial_port(&self, name: &str) -> bool {
        name.to_ascii_uppercase().starts_with("COM")
    }

    fn base_data_dir(&self) -> PathBuf {
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        base.join("pawflash")
    }

    fn bridge_binary_name(&self) -> &'static str {
        "bridge.exe"
    }

    fn manifest_platform_key(&self) -> Result<&'static str, String> {
        if cfg!(target_arch = "x86_64") {
            Ok("windows-x86_64")
        } else {
            Err(format!("unsupported architecture: {}", std::env::consts::ARCH))
        }
    }

    fn ensure_driver(&self) -> Result<(), String> {
        if self.usbdk_installed() {
            return Ok(());
        }

        let usbdk_url = "https://github.com/daynix/UsbDk/releases/download/v1.00-22/UsbDk_1.0.22_x64.msi";
        let temp_dir = std::env::temp_dir();
        let msi_path = temp_dir.join("UsbDk_1.0.22_x64.msi");

        if !msi_path.exists() {
            let mut response = ureq::get(usbdk_url)
                .call()
                .map_err(|e| format!("failed to download USBDK: {e}"))?;

            let mut file = std::fs::File::create(&msi_path)
                .map_err(|e| format!("failed to create temp file: {e}"))?;

            let body = response.body_mut();
            let mut reader = body.as_reader();
            let mut buf = [0u8; 8192];
            loop {
                let n = reader
                    .read(&mut buf)
                    .map_err(|e| format!("failed to read USBDK download: {e}"))?;
                if n == 0 {
                    break;
                }
                std::io::Write::write_all(&mut file, &buf[..n])
                    .map_err(|e| format!("failed to write USBDK installer: {e}"))?;
            }
        }

        let _ = std::process::Command::new("msiexec")
            .args([
                "/i",
                msi_path.to_str().unwrap_or_default(),
                "/qn",
                "/norestart",
            ])
            .status();

        let _ = std::fs::remove_file(&msi_path);

        if self.usbdk_installed() {
            return Ok(());
        }
        Err(format!(
            "USBDK is required on Windows. Install it from:\n  {usbdk_url}\n\
             then run `pawflash mtkclient doctor` again."
        ))
    }

    fn usbdk_installed(&self) -> bool {
        std::process::Command::new("sc")
            .args(["query", "UsbDk"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn permission_error_patterns(&self) -> &'static [&'static str] {
        &["permission denied", "access is denied", "access denied"]
    }

    fn accept_unknown_port_type(&self) -> bool {
        true
    }

    fn install_udev_rules(&self) -> bool {
        // No udev on Windows.
        true
    }

    fn add_user_to_group(&self) -> bool {
        // No usermod on Windows.
        false
    }

    fn print_manual_guidance(&self) {
        tracing::warn!(
            "Permission denied opening serial port on Windows.\n\
             Ensure the preloader is enumerated as a COM port (MediaTek USB VCOM / usbser driver),\
             not as a WinUSB device — this flow talks to the preloader over a serial port."
        );
    }

    fn post_handshake_hint(&self) -> &'static str {
        "Device left preloader. If it does not appear in fastboot, verify Google USB Driver (android_winusb) or WinUSB driver is installed for the device."
    }

    fn fastboot_interface_openable(&self, info: &fastboot_protocol::nusb::DeviceInfo) -> bool {
        info.driver().is_some_and(|d| {
            d.eq_ignore_ascii_case("winusb")
                || d.eq_ignore_ascii_case("androidwinusb")
                || d.eq_ignore_ascii_case("androidwinusb86")
                || d.eq_ignore_ascii_case("androidusb")
                || d.eq_ignore_ascii_case("wudfrd")
        })
    }

    fn fastboot_driver_name(
        &self,
        info: &fastboot_protocol::nusb::DeviceInfo,
    ) -> Option<String> {
        info.driver().map(str::to_owned)
    }
}
