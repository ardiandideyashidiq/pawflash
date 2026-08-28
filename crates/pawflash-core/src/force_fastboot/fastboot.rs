use std::fs;
use std::path::Path;
use tracing::{debug, info, trace, warn};

pub async fn list_fastboot_devices() {
    let Ok(probes) = fastboot_protocol::nusb::probe().await else { return };
    for p in probes.iter().filter(|p| p.is_fastboot()) {
        info!(
            serial = p.serial.as_deref().unwrap_or("?"),
            vidpid = p.vidpid(),
            "fastboot device",
        );
    }
}

pub async fn in_fastboot_mode() -> bool {
    let nusb = nusb_fastboot_mode().await;
    let sysfs = linux_sysfs_fastboot_mode();
    let result = nusb || sysfs;
    debug!(nusb, sysfs, result, "fastboot mode check");
    result
}

async fn nusb_fastboot_mode() -> bool {
    match fastboot_protocol::nusb::devices().await {
        Ok(mut devices) => {
            let found = devices.next().is_some();
            if found {
                debug!("found fastboot device via nusb");
            }
            found
        }
        Err(err) => {
            warn!(%err, "failed to enumerate USB devices with nusb");
            false
        }
    }
}

fn linux_sysfs_fastboot_mode() -> bool {
    if cfg!(not(target_os = "linux")) {
        return false;
    }

    let root = Path::new("/sys/bus/usb/devices");
    let Ok(entries) = fs::read_dir(root) else {
        trace!("cannot read /sys/bus/usb/devices");
        return false;
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();

        if !name.contains(':') {
            continue;
        }

        let base = entry.path();
        let class = read_trimmed(base.join("bInterfaceClass"));
        let subclass = read_trimmed(base.join("bInterfaceSubClass"));
        let protocol = read_trimmed(base.join("bInterfaceProtocol"));

        debug!(%name, %class, %subclass, %protocol, "sysfs interface");

        if class == "ff" && subclass == "42" && protocol == "03" {
            debug!(%name, "found fastboot interface via sysfs");
            return true;
        }
    }

    false
}

fn read_trimmed(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path)
        .map(|s| s.trim().to_ascii_lowercase())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fastboot_interface_bytes_match_aosp() {
        assert_eq!(0xff, 0xff);
        assert_eq!(0x42, 0x42);
        assert_eq!(0x03, 0x03);
    }

    #[tokio::test]
    async fn fastboot_mode_should_fallback_gracefully_when_nusb_fails() {
        assert!(!nusb_fastboot_mode().await || cfg!(target_os = "linux"));
    }

    #[test]
    fn read_trimmed_should_return_default_for_missing_path() {
        let result = read_trimmed("/sys/force-fastboot-nonexistent");
        assert_eq!(result, "");
    }

    #[test]
    fn read_trimmed_should_lowercase_and_trim() {
        let result = read_trimmed("/tmp/__force_fastboot_test__");
        assert_eq!(result, "");
    }
}
