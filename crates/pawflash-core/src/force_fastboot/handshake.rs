//! FASTBOOT preloader handshake — the shared implementation used by both the
//! CLI and the Tauri GUI.
//!
//! The handshake repeatedly writes `FASTBOOT` to the preloader serial port,
//! polls for fastboot mode over USB, and reconnects if the port drops. It was
//! previously written twice (CLI + GUI) with diverging bookkeeping; both UIs
//! now call [`handshake`] and render [`HandshakeEvent`]s differently.

use std::sync::atomic::{AtomicBool, Ordering};

use tokio::io::AsyncWriteExt;
use tokio::time::{sleep, Duration};
use tracing::{debug, warn};

use super::error::{Error, Result};
use super::fastboot::in_fastboot_mode;
use super::serial::{open_with_permission_recovery, wait_for_reconnect};

/// Delay between FASTBOOT writes.
const WRITE_INTERVAL: Duration = Duration::from_millis(500);

/// How long to wait for the preloader port to reappear after it drops before
/// assuming the device has left preloader (entered fastboot mode).
const RECONNECT_WINDOW: Duration = Duration::from_secs(10);

/// Progress events emitted during the handshake, so UIs can render progress
/// without duplicating the loop logic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandshakeEvent {
    /// One `FASTBOOT` write completed.
    Write { count: u64 },
    /// The serial port dropped and the handshake is waiting to reconnect.
    PortLost { port: String },
    /// Reconnected on a new preloader port.
    PortReconnected { port: String },
}

/// Drive the FASTBOOT preloader handshake until the device enters fastboot
/// mode. Repeatedly writes `FASTBOOT` to `dev`, polls USB fastboot mode, and
/// reconnects via [`wait_for_reconnect`] if the port drops.
///
/// Returns the number of `FASTBOOT` writes sent.
///
/// # Errors
///
/// Returns an error if the preloader port cannot be reopened after a drop, if
/// it was lost before any write completed, or if serial enumeration fails.
pub async fn handshake(
    mut dev: tokio_serial::SerialStream,
    initial_port: &str,
    cancel: Option<&AtomicBool>,
    mut on_event: impl FnMut(HandshakeEvent),
) -> Result<u64> {
    let mut count: u64 = 0;
    let mut port = initial_port.to_string();

    loop {
        if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            debug!(sends = count, "handshake cancelled");
            break;
        }

        match dev.write_all(b"FASTBOOT").await {
            Ok(()) => {
                let _ = dev.flush().await;
                count += 1;
                on_event(HandshakeEvent::Write { count });
                if count % 5 == 0 {
                    debug!(sends = count, "batch progress");
                }
            }
            Err(err) => {
                warn!(%err, sends = count, "serial write failed");
                on_event(HandshakeEvent::PortLost { port: port.clone() });

                if in_fastboot_mode().await {
                    debug!("fastboot mode detected after write failure");
                    break;
                }

                drop(dev);
                if let Some(new_port) = wait_for_reconnect(RECONNECT_WINDOW, cancel).await? {
                    debug!(port = %new_port, "reconnected after port loss");
                    port = new_port;
                    on_event(HandshakeEvent::PortReconnected { port: port.clone() });
                    dev = open_with_permission_recovery(&port)?;
                    continue;
                }
                if count == 0 {
                    return Err(Error::PortLostBeforeWrite { port: port.clone() });
                }
                debug!(sends = count, "preloader port did not reappear — device left preloader");
                break;
            }
        }

        if in_fastboot_mode().await {
            debug!(sends = count, "fastboot mode detected");
            break;
        }

        sleep(WRITE_INTERVAL).await;
    }

    Ok(count)
}
