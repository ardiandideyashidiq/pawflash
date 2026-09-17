use tracing::{debug, info, warn};

pub async fn list_fastboot_devices() {
    let Ok(probes) = fastboot_protocol::nusb::probe().await else { return };
    for p in probes.iter().filter(|p| p.is_fastboot()) {
        #[cfg(target_os = "windows")]
        info!(
            serial = p.serial.as_deref().unwrap_or("?"),
            vidpid = p.vidpid(),
            driver = p.driver.as_deref().unwrap_or("<none>"),
            product = p.product.as_deref().unwrap_or("<none>"),
            "fastboot device",
        );
        #[cfg(not(target_os = "windows"))]
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
            if let Some(dev) = devices.next() {
                #[cfg(target_os = "windows")]
                debug!(
                    vidpid = format_args!("{:04x}:{:04x}", dev.vendor_id(), dev.product_id()),
                    serial = dev.serial_number().unwrap_or("?"),
                    driver = dev.driver().unwrap_or("<none>"),
                    product = dev.product_string().unwrap_or("<none>"),
                    "fastboot mode check: device confirmed",
                );
                #[cfg(not(target_os = "windows"))]
                debug!(
                    vidpid = format_args!("{:04x}:{:04x}", dev.vendor_id(), dev.product_id()),
                    serial = dev.serial_number().unwrap_or("?"),
                    "fastboot mode check: device confirmed",
                );
                true
            } else {
                debug!("fastboot mode check: no device matched");
                false
            }
        }
        Err(err) => {
            warn!(%err, "failed to enumerate USB devices with nusb");
            false
        }
    }
}

/// Diagnostic inspection of connected USB devices with Windows-specific driver details.
/// Call this during force fastboot transitions or when detection fails.
pub async fn log_fastboot_diagnostics() {
    let probes = match fastboot_protocol::nusb::probe().await {
        Ok(p) => p,
        Err(err) => {
            warn!(%err, "failed to probe USB devices for diagnostics");
            return;
        }
    };

    debug!(total_usb_devices = probes.len(), "probing USB devices for fastboot diagnostics");

    let fastboot_devices: Vec<_> = probes.iter().filter(|p| p.is_fastboot()).collect();
    if !fastboot_devices.is_empty() {
        for p in &fastboot_devices {
            #[cfg(target_os = "windows")]
            info!(
                vidpid = p.vidpid(),
                serial = p.serial.as_deref().unwrap_or("?"),
                driver = p.driver.as_deref().unwrap_or("<none>"),
                product = p.product.as_deref().unwrap_or("<none>"),
                "detected fastboot device on Windows",
            );
            #[cfg(not(target_os = "windows"))]
            info!(
                vidpid = p.vidpid(),
                serial = p.serial.as_deref().unwrap_or("?"),
                "detected fastboot device",
            );
        }
        return;
    }

    // No fastboot device was recognized: inspect candidate mobile or MediaTek devices.
    let candidates: Vec<_> = probes
        .iter()
        .filter(|p| {
            crate::flash::executor::connect::is_candidate_mobile_device(p.vid)
                || p.product.as_deref().is_some_and(|s| {
                    let lower = s.to_ascii_lowercase();
                    lower.contains("fastboot")
                        || lower.contains("bootloader")
                        || lower.contains("mediatek")
                        || lower.contains("mt65")
                        || lower.contains("android")
                })
        })
        .collect();

    if candidates.is_empty() {
        debug!("no candidate mobile or fastboot USB devices found during diagnostic probe");
    } else {
        for p in &candidates {
            #[cfg(target_os = "windows")]
            {
                let drv = p.driver.as_deref().unwrap_or("<none>");
                let prod = p.product.as_deref().unwrap_or("<none>");
                let is_openable = p.driver.as_deref().is_some_and(|d| {
                    d.eq_ignore_ascii_case("winusb")
                        || d.eq_ignore_ascii_case("androidwinusb")
                        || d.eq_ignore_ascii_case("androidwinusb86")
                        || d.eq_ignore_ascii_case("androidusb")
                });

                if is_openable {
                    info!(
                        vidpid = p.vidpid(),
                        serial = p.serial.as_deref().unwrap_or("?"),
                        driver = drv,
                        product = prod,
                        "USB candidate device has valid WinUSB driver but was not detected as fastboot",
                    );
                } else {
                    warn!(
                        vidpid = p.vidpid(),
                        serial = p.serial.as_deref().unwrap_or("?"),
                        driver = drv,
                        product = prod,
                        "USB candidate device found but requires WinUSB or Android Bootloader Interface driver",
                    );
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                debug!(
                    vidpid = p.vidpid(),
                    serial = p.serial.as_deref().unwrap_or("?"),
                    product = p.product.as_deref().unwrap_or("<none>"),
                    "USB candidate device found but did not match fastboot interface",
                );
            }
        }
    }
}

