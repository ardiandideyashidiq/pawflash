use tracing::{debug, info, warn};

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

/// Whether any connected USB device exposes a fastboot interface.
///
/// This mirrors the official Android fastboot detection: enumerate USB
/// devices and match the fastboot interface class triple (0xff / 0x42 / 0x03).
/// Enumeration goes through `nusb`, which on every supported OS reads the USB
/// descriptors directly, so this works without any driver setup on Windows.
pub async fn in_fastboot_mode() -> bool {
    match fastboot_protocol::nusb::devices().await {
        Ok(mut devices) => {
            let found = devices.next().is_some();
            debug!(found, "fastboot mode check");
            found
        }
        Err(err) => {
            warn!(%err, "failed to enumerate USB devices with nusb");
            false
        }
    }
}
