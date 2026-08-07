use super::error::Error;
use super::error::Result;
use super::fastboot::in_fastboot_mode;
use super::{permissions, udev};
use inquire::Confirm;
use std::collections::HashSet;
use std::io::IsTerminal;
use tokio::time::{sleep, Duration};
use tracing::{debug, info, trace, warn};

const BAUD: u32 = 115_200;
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const PORT_TIMEOUT: Duration = Duration::from_millis(250);

#[must_use]
pub fn serial_ports() -> HashSet<String> {
    let ports = match tokio_serial::available_ports() {
        Ok(ports) => ports,
        Err(err) => {
            warn!(%err, "failed to enumerate serial ports");
            return HashSet::new();
        }
    };

    ports
        .into_iter()
        .filter_map(|p| {
            if is_candidate_port(&p) {
                Some(p.port_name)
            } else {
                trace!(port = %p.port_name, "skipping non-candidate serial port");
                None
            }
        })
        .collect()
}

fn is_candidate_serial_port(name: &str) -> bool {
    if cfg!(target_os = "windows") {
        name.to_ascii_uppercase().starts_with("COM")
    } else if cfg!(target_os = "linux") {
        name.starts_with("/dev/ttyACM") || name.starts_with("/dev/ttyUSB")
    } else {
        false
    }
}

/// A port is a candidate if its name looks like a preloader port AND, for
/// `ttyUSB` adapters, it reports a `MediaTek` VID when one is present. This
/// keeps unrelated USB-serial adapters (`FTDI`, `CP210x`, `CH340`) from being
/// hammered with FASTBOOT handshakes.
fn is_candidate_port(info: &tokio_serial::SerialPortInfo) -> bool {
    if !is_candidate_serial_port(&info.port_name) {
        return false;
    }
    if info.port_name.starts_with("/dev/ttyACM") || cfg!(target_os = "windows") {
        return true;
    }
    match &info.port_type {
        tokio_serial::SerialPortType::UsbPort(usb) if usb.vid != 0x0e8d => {
            trace!(
                port = %info.port_name,
                vid = format_args!("{:04x}", usb.vid),
                "skipping non-MediaTek USB serial adapter",
            );
            false
        }
        _ => true,
    }
}

/// Open a serial port to the preloader.
///
/// # Errors
///
/// Returns an error if the port cannot be opened.
pub fn open_serial(port: &str) -> Result<tokio_serial::SerialStream> {
    use tokio_serial::SerialPortBuilderExt;
    debug!(%port, baud = BAUD, "opening serial port");
    tokio_serial::new(port, BAUD)
        .timeout(PORT_TIMEOUT)
        .open_native_async()
        .map_err(|source| Error::OpenSerialPort {
            port: port.to_owned(),
            source,
        })
        .inspect(|_| info!(%port, "serial port opened"))
}

/// Open a serial port with automatic permission recovery.
///
/// On permission denied, attempts to install udev rules and add the user
/// to the dialout group before retrying.
///
/// # Errors
///
/// Returns an error if the port cannot be opened even after recovery
/// attempts.
pub fn open_with_permission_recovery(port: &str) -> Result<tokio_serial::SerialStream> {
    match open_serial(port) {
        Ok(stream) => return Ok(stream),
        Err(err) => {
            if !permissions::is_permission_error(&err) {
                return Err(err);
            }
        }
    }

    warn!(%port, "permission denied — attempting recovery");

    // Interactive prompts need a controlling terminal. Outside one (e.g. the
    // Tauri GUI, where there is no TTY) skip the recovery dialogs entirely and
    // surface the original permission error so the caller can show it.
    if !std::io::stdin().is_terminal() {
        warn!("stdin is not a terminal — skipping interactive permission-recovery prompts");
        udev::print_manual_guidance();
        return open_serial(port);
    }

    // Prompt before installing udev rules (default no — opt-in).
    if Confirm::new("Permission denied. Install udev rules for MediaTek preloader? (requires sudo)")
        .with_default(false)
        .prompt()
        .unwrap_or(false)
        && udev::install_udev_rules()
    {
        if let Ok(stream) = open_serial(port) {
            info!(%port, "reconnected after udev rule install");
            return Ok(stream);
        }
    }

    // Prompt before adding user to dialout group (default no — opt-in).
    if Confirm::new("Add current user to dialout/plugdev groups? (requires sudo, log out/in to take effect)")
        .with_default(false)
        .prompt()
        .unwrap_or(false)
        && udev::add_user_to_group()
    {
        if let Ok(stream) = open_serial(port) {
            info!(%port, "reconnected after group add");
            return Ok(stream);
        }
    }

    udev::print_manual_guidance();

    // Re-wrap the original error
    open_serial(port)
}

/// Whether an already-present preloader port is a match given whether the
/// device is already in fastboot mode.
///
/// Fastboot-mode detection wins: when checking for fastboot, a device already
/// in fastboot no longer exposes a preloader port, so an existing port is not
/// treated as a match.
const fn should_match_existing(check_fastboot: bool, in_fastboot: bool) -> bool {
    !(check_fastboot && in_fastboot)
}

/// Wait for a new preloader serial port to appear.
///
/// An already-present candidate port (in the set at entry) is matched
/// immediately, unless the device is detected to already be in fastboot.
///
/// # Errors
///
/// Returns an error if serial port enumeration fails or the timeout (120s)
/// is exceeded.
pub async fn wait_for_preloader(
    check_fastboot: bool,
) -> Result<Option<String>> {
    info!(check_fastboot, "waiting for preloader serial port (max 120s)");
    let initial = serial_ports();
    let mut old = initial.clone();
    let mut iterations = 0u64;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);

    loop {
        if tokio::time::Instant::now() >= deadline {
            warn!("timed out waiting for preloader serial port after 120s");
            return Err(Error::PreloaderTimeout);
        }

        iterations += 1;
        trace!(iterations, ports = ?old, "polling for new serial port");

        let in_fastboot = check_fastboot && in_fastboot_mode().await;
        if !should_match_existing(check_fastboot, in_fastboot) {
            info!("fastboot detected while waiting for preloader, returning None");
            return Ok(None);
        }

        // If we are not already in fastboot, an existing preloader port is a
        // match — do not require it to appear after we started polling.
        if let Some(port) = initial.iter().next().cloned() {
            info!(%port, "preloader port already present");
            return Ok(Some(port));
        }

        let new = serial_ports();

        if let Some(port) = new.difference(&old).next().cloned() {
            info!(%port, iterations, "new preloader serial port appeared");
            return Ok(Some(port));
        }

        if old.difference(&new).next().is_some() {
            debug!("serial port set changed, refreshing baseline");
            old = new;
        }

        sleep(POLL_INTERVAL).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_match_existing_pins_fastboot_precedence() {
        assert!(!should_match_existing(true, true), "in fastboot -> existing port is not a match");
        assert!(should_match_existing(true, false), "not in fastboot -> existing port is a match");
        assert!(should_match_existing(false, true), "fastboot not checked -> existing port is a match");
        assert!(should_match_existing(false, false));
    }

    #[test]
    fn is_candidate_serial_port_should_accept_linux_acm() {
        assert!(is_candidate_serial_port("/dev/ttyACM0"));
    }

    #[test]
    fn is_candidate_serial_port_should_reject_bogus_linux_path() {
        assert!(!is_candidate_serial_port("/dev/ttyS0"), "ttyS is not a preloader candidate");
    }

    #[test]
    fn is_candidate_serial_port_should_reject_empty() {
        assert!(!is_candidate_serial_port(""));
    }

    #[test]
    fn serial_ports_should_not_panic_when_no_ports() {
        let _ports = serial_ports();
    }

    #[tokio::test]
    async fn open_serial_should_error_on_bogus_port() {
        let err = open_serial("/dev/__force_fastboot_nonexistent__").unwrap_err();
        assert!(
            err.to_string().contains("/dev/__force_fastboot_nonexistent__"),
            "error should mention the port name: {err}",
        );
    }
}
