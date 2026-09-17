use std::collections::HashMap;
use std::time::Duration;

use fastboot_protocol::nusb::{InterfaceKind, NusbFastBoot, NusbFastBootOpenError};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::flash::error::{FlashError, Result};
use crate::platform;
use super::{expected_serial, FlashExecutor};

/// Classify why no fastboot device matched: ADB mode, unknown USB devices, or
/// nothing connected at all.
async fn no_device_error(expected: Option<&str>) -> FlashError {
    let Ok(probes) = fastboot_protocol::nusb::probe().await else {
        return FlashError::NoDevice;
    };
    classify_no_device(&probes, expected)
}

pub(crate) const fn is_candidate_mobile_device(vid: u16) -> bool {
    matches!(
        vid,
        0x18d1 // Google
        | 0x2717 // Xiaomi
        | 0x0e8d // MediaTek
        | 0x05c6 // Qualcomm
        | 0x04e8 // Samsung
        | 0x12d1 // Huawei
        | 0x22d9 // OPPO / OnePlus / Realme
        | 0x2d95 // Vivo
        | 0x22b8 // Motorola
        | 0x1004 // LG
        | 0x0b05 // Asus
        | 0x17ef // Lenovo
        | 0x1782 // Spreadtrum / Unisoc
    )
}

/// Decide the detection diagnostic from a set of USB device probes.
fn classify_no_device(probes: &[fastboot_protocol::nusb::Probe], expected: Option<&str>) -> FlashError {
    let adb_serials: Vec<String> = probes
        .iter()
        .filter(|p| p.kind == InterfaceKind::Adb)
        .filter(|p| expected.is_none_or(|exp| p.serial.as_deref() == Some(exp)))
        .filter_map(|p| p.serial.clone())
        .collect();
    if !adb_serials.is_empty() {
        return FlashError::DeviceInAdb { serials: adb_serials };
    }

    let vids: Vec<String> = probes
        .iter()
        .filter(|p| is_candidate_mobile_device(p.vid))
        .map(fastboot_protocol::nusb::Probe::vidpid)
        .collect();
    if !vids.is_empty() {
        return FlashError::NoUsbInterface { vids };
    }

    FlashError::NoDevice
}

/// Query essential variables needed for device identification and flashing.
/// Fastbootd vs bootloader mode is determined by querying `is-userspace` first.
async fn query_essential_vars(
    fb: &mut NusbFastBoot,
) -> (HashMap<String, String>, Vec<String>, Duration) {
    let mut device_vars = HashMap::new();
    let mut slow_vars = Vec::new();
    let t_vars = std::time::Instant::now();
    for var in [
        "is-userspace",
        "version",
        "version-bootloader",
        "product",
        "serialno",
        "current-slot",
        "unlocked",
        "secure",
        "max-download-size",
    ] {
        let t_var = std::time::Instant::now();
        match tokio::time::timeout(Duration::from_millis(500), fb.get_var(var)).await {
            Ok(Ok(v)) => {
                let d = t_var.elapsed();
                if d >= Duration::from_millis(50) {
                    slow_vars.push(format!("{var}={v} ({d:?})"));
                    debug!(var, value = %v, ?d, "slow device var query");
                } else {
                    debug!(var, value = %v, ?d, "queried device var");
                }
                if !v.trim().is_empty() {
                    device_vars.insert(var.to_string(), v.trim().to_string());
                }
            }
            Ok(Err(e)) => {
                let d = t_var.elapsed();
                if d >= Duration::from_millis(50) {
                    slow_vars.push(format!("{var}=rejected ({d:?})"));
                }
                debug!(var, ?d, error = %e, "device var query rejected");
            }
            Err(_) => {
                let d = t_var.elapsed();
                slow_vars.push(format!("{var}=timeout ({d:?})"));
                warn!(var, ?d, "device var query timed out after 500ms");
            }
        }
    }
    (device_vars, slow_vars, t_vars.elapsed())
}

impl FlashExecutor<NusbFastBoot> {
    /// # Errors
    /// Returns `NoDevice` if no fastboot device is found,
    /// `MultipleDevices` if several devices match and no serial pins one, or
    /// `DeviceMismatch` if the device serial does not match the expected value.
    pub async fn connect() -> Result<Self> {
        let expected = expected_serial();
        let all: Vec<_> = fastboot_protocol::nusb::devices()
            .await
            .map_err(|_| FlashError::NoDevice)?
            .filter(|info| expected.is_none_or(|exp| info.serial_number() == Some(exp)))
            .collect();
        if all.len() > 1 {
            let serials: Vec<String> = all
                .iter()
                .filter_map(|info| info.serial_number().map(str::to_owned))
                .collect();
            return Err(FlashError::MultipleDevices { serials });
        }
        let Some(info) = all.into_iter().next() else {
            return Err(no_device_error(expected).await);
        };
        debug!(
            vidpid = format_args!("{:04x}:{:04x}", info.vendor_id(), info.product_id()),
            serial = info.serial_number().unwrap_or("?"),
            "connecting to fastboot device"
        );
        let t0 = std::time::Instant::now();
        let mut fb = match NusbFastBoot::from_info(&info).await {
            Ok(fb) => fb,
            Err(e) => {
                let is_incompatible = match &e {
                    NusbFastBootOpenError::Interface(err) | NusbFastBootOpenError::Device(err) => {
                        err.to_string().contains("incompatible driver")
                    }
                    _ => false,
                };
                if is_incompatible || !platform::CURRENT.fastboot_interface_openable(&info) {
                    let vidpid = format!("{:04x}:{:04x}", info.vendor_id(), info.product_id());
                    let serial = info.serial_number().map(str::to_owned);
                    let driver = platform::CURRENT.fastboot_driver_name(&info);
                    return Err(FlashError::UnsupportedDriver { vidpid, driver, serial });
                }
                return Err(FlashError::Open(e));
            }
        };
        let open_duration = t0.elapsed();
        debug!(?open_duration, "fastboot USB device opened and interface claimed");

        let (mut device_vars, slow_vars, vars_duration) = query_essential_vars(&mut fb).await;

        if !device_vars.contains_key("serialno") {
            if let Some(sn) = info.serial_number() {
                device_vars.insert("serialno".to_string(), sn.to_string());
            }
        }

        if !device_vars.contains_key("product") {
            if let Some(p) = info.product_string() {
                device_vars.insert("product".to_string(), p.to_string());
            }
        }

        if let Some(expected) = expected {
            match device_vars.get("serialno").map(String::as_str) {
                Some(s) if s == expected => {
                    debug!(serial = %s, "device serial matches expected");
                }
                Some(s) => {
                    return Err(FlashError::DeviceMismatch {
                        expected: expected.to_string(),
                        actual: s.to_string(),
                    });
                }
                None => {
                    warn!("--serial set but device did not report serialno; proceeding");
                }
            }
        }
        let is_userspace_str = device_vars.get("is-userspace").map_or("no", |s| s.as_str());
        if slow_vars.is_empty() {
            info!(
                ?open_duration,
                ?vars_duration,
                total = ?t0.elapsed(),
                product = device_vars.get("product").map_or("?", |s| s.as_str()),
                serial = device_vars.get("serialno").map_or("?", |s| s.as_str()),
                version = device_vars.get("version").map_or("?", |s| s.as_str()),
                is_userspace = is_userspace_str,
                "connected to fastboot device"
            );
        } else {
            info!(
                ?open_duration,
                ?vars_duration,
                total = ?t0.elapsed(),
                product = device_vars.get("product").map_or("?", |s| s.as_str()),
                serial = device_vars.get("serialno").map_or("?", |s| s.as_str()),
                version = device_vars.get("version").map_or("?", |s| s.as_str()),
                is_userspace = is_userspace_str,
                slow_vars = ?slow_vars,
                "connected to fastboot device (slow/unsupported queries noted)"
            );
        }
        Ok(Self { fb, device_vars, max_download: None })
    }

    /// Wait for a fastboot device to reappear after reboot.
    ///
    /// # Errors
    ///
    /// Returns `NoDevice` if no fastboot device appears within the timeout,
    /// or if the provided `cancel` token is fired.
    pub async fn wait_for_device(
        timeout: Duration,
        cancel: CancellationToken,
    ) -> Result<Self> {
        let start = std::time::Instant::now();
        let mut last_log = start;
        loop {
            if start.elapsed() > timeout {
                return Err(FlashError::NoDevice);
            }
            if cancel.is_cancelled() {
                return Err(FlashError::NoDevice);
            }
            match Self::connect().await {
                Ok(executor) => return Ok(executor),
                Err(e) => {
                    if last_log.elapsed() > Duration::from_secs(5) {
                        warn!("waiting for fastboot device after reboot (error: {e}) ...");
                        last_log = std::time::Instant::now();
                    }
                    tokio::select! {
                        () = cancel.cancelled() => {
                            return Err(FlashError::NoDevice);
                        }
                        () = tokio::time::sleep(Duration::from_millis(250)) => {}
                    }
                }
            }
        }
    }

}

#[cfg(test)]
mod tests {
    use fastboot_protocol::nusb::{InterfaceKind, Probe};

    use super::classify_no_device;
    use crate::flash::error::FlashError;

    fn probe(vid: u16, pid: u16, serial: Option<&str>, kind: InterfaceKind) -> Probe {
        Probe {
            vid,
            pid,
            serial: serial.map(str::to_owned),
            kind,
            iface_count: 1,
            driver: None,
            product: None,
        }
    }

    #[test]
    fn no_devices_is_no_device() {
        assert!(matches!(
            classify_no_device(&[], None),
            FlashError::NoDevice
        ));
    }

    #[test]
    fn adb_device_yields_adb_error_with_serial() {
        let probes = vec![probe(0x18d1, 0x4ee2, Some("abcd1234"), InterfaceKind::Adb)];
        assert!(matches!(
            classify_no_device(&probes, None),
            FlashError::DeviceInAdb { ref serials } if serials == &["abcd1234".to_string()]
        ));
    }

    #[test]
    fn adb_device_filtered_by_expected_serial_drops_to_vids() {
        let probes = vec![probe(0x18d1, 0x4ee2, Some("abcd1234"), InterfaceKind::Adb)];
        assert!(matches!(
            classify_no_device(&probes, Some("other")),
            FlashError::NoUsbInterface { ref vids } if vids == &["18d1:4ee2".to_string()]
        ));
    }

    #[test]
    fn unknown_usb_device_yields_no_interface_error() {
        let probes = vec![probe(0x0e8d, 0x2000, None, InterfaceKind::Other)];
        assert!(matches!(
            classify_no_device(&probes, None),
            FlashError::NoUsbInterface { ref vids } if vids == &["0e8d:2000".to_string()]
        ));
    }

    #[test]
    fn present_devices_yield_interface_diagnostics() {
        // classify_no_device is only consulted when fastboot enumeration found
        // nothing, so any present probe is reported via NoUsbInterface.
        let probes = vec![probe(0x18d1, 0x4ee0, Some("fastserial"), InterfaceKind::Fastboot)];
        assert!(matches!(
            classify_no_device(&probes, None),
            FlashError::NoUsbInterface { .. }
        ));
    }

    #[test]
    fn unrelated_peripheral_yields_no_device() {
        let probes = vec![probe(0x046d, 0xc539, None, InterfaceKind::Other)];
        assert!(matches!(
            classify_no_device(&probes, None),
            FlashError::NoDevice
        ));
    }

    #[test]
    fn unrelated_composite_peripheral_yields_no_device() {
        let mut p = probe(0x046d, 0xc539, None, InterfaceKind::Other);
        p.driver = Some("usbccgp".to_string());
        p.product = Some("Logitech USB Receiver".to_string());
        assert!(matches!(
            classify_no_device(&[p], None),
            FlashError::NoDevice
        ));
    }
}
