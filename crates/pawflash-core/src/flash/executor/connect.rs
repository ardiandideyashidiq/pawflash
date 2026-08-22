use std::collections::HashMap;
use std::time::Duration;

use fastboot_protocol::nusb::{InterfaceKind, NusbFastBoot};
#[cfg(target_os = "windows")]
use fastboot_protocol::nusb::NusbFastBootOpenError;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::flash::error::{FlashError, Result};
use super::{BootTarget, expected_serial, FlashExecutor};

/// Classify why no fastboot device matched: ADB mode, unknown USB devices, or
/// nothing connected at all.
async fn no_device_error(expected: Option<&str>) -> FlashError {
    let Ok(probes) = fastboot_protocol::nusb::probe().await else {
        return FlashError::NoDevice;
    };
    classify_no_device(&probes, expected)
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

    let vids: Vec<String> = probes.iter().map(fastboot_protocol::nusb::Probe::vidpid).collect();
    if !vids.is_empty() {
        return FlashError::NoUsbInterface { vids };
    }

    FlashError::NoDevice
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
        let mut fb = match NusbFastBoot::from_info(&info).await {
            Ok(fb) => fb,
            Err(e) => {
                #[cfg(target_os = "windows")]
                if matches!(e, NusbFastBootOpenError::Interface(_) | NusbFastBootOpenError::Device(_)) {
                    let vidpid = format!("{:04x}:{:04x}", info.vendor_id(), info.product_id());
                    let driver = info.driver().map(str::to_owned);
                    let serial = info.serial_number().map(str::to_owned);
                    return Err(FlashError::WindowsDriver { vidpid, driver, serial });
                }
                return Err(FlashError::Open(e));
            }
        };
        let device_vars = match tokio::time::timeout(
            std::time::Duration::from_secs(10),
            fb.get_all_vars(),
        )
            .await
        {
            Ok(Ok(vars)) => vars,
            Ok(Err(e)) => {
                debug!(error = %e, "getvar:all failed, falling back to individual queries");
                HashMap::new()
            }
            Err(_) => {
                debug!("getvar:all timed out, falling back to individual queries");
                HashMap::new()
            }
        };
        let device_vars = if device_vars.is_empty() {
            let mut vars: HashMap<String, String> = HashMap::new();
            for var in ["version", "product", "serialno", "current-slot", "max-download-size"] {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    fb.get_var(var),
                )
                    .await
                {
                    Ok(Ok(v)) => { vars.insert(var.to_string(), v); }
                    Ok(Err(e)) => { debug!(%var, error = %e, "getvar failed"); }
                    Err(_) => { debug!(%var, "getvar timed out"); }
                }
            }
            vars
        } else {
            device_vars
        };
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
        info!(
            product = device_vars.get("product").map_or("?", |s| s.as_str()),
            serial = device_vars.get("serialno").map_or("?", |s| s.as_str()),
            version = device_vars.get("version").map_or("?", |s| s.as_str()),
            "connected to fastboot device"
        );
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

    /// # Errors
    /// Returns an error if the device does not reappear within 120 seconds.
    pub async fn reboot_and_wait(mut self, target: BootTarget) -> Result<Self> {
        debug!(?target, "rebooting device and waiting for reconnect");
        if let Err(e) = self.fb.reboot_to(target.as_str()).await {
            warn!(?target, error = %e, "reboot command error (device may have disconnected)");
        }
        drop(self);
        Self::wait_for_device(Duration::from_secs(120), CancellationToken::default()).await
    }

    /// # Errors
    /// Returns an error if the device cannot transition to fastbootd.
    pub async fn ensure_fastbootd(mut self) -> Result<Self> {
        let is_fastbootd = self.fb.get_var("is-userspace").await.is_ok_and(|v| v == "yes");
        if is_fastbootd {
            debug!("already in fastbootd mode");
            return Ok(self);
        }
        info!("device is in bootloader mode, rebooting to fastbootd");
        self.reboot_and_wait(BootTarget::Fastboot).await
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
            #[cfg(target_os = "windows")]
            driver: None,
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
}
