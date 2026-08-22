use miette::Result;
use tokio::time::{sleep, Duration, Instant};
use tracing::{debug, info, trace};

use pawflash_core::force_fastboot::{fastboot, handshake, serial};
use pawflash_core::output;

/// Force a `MediaTek` device into fastboot mode via preloader handshake.
///
/// When `simulate` is true, replays a realistic 10-second handshake
/// without any serial port or USB interaction.
///
/// # Errors
///
/// Returns an error if no preloader serial port is found, the serial
/// port cannot be opened, or the handshake otherwise fails.
pub async fn run(simulate: bool) -> Result<()> {
    if simulate {
        return run_simulated().await;
    }
    let start_all = Instant::now();
    info!("starting");

    output::status::heading("Scanning USB for fastboot devices...");
    if fastboot::in_fastboot_mode().await {
        output::status::ok("[+]", "fastboot mode detected");
        fastboot::list_fastboot_devices().await;
        info!(total_secs = start_all.elapsed().as_secs_f32(), sends = 0u64, "force-fastboot complete");
        return Ok(());
    }

    let Some(port) = output::spinner::run_with_spinner(
        "Waiting for preloader serial port (120s timeout)...",
        serial::wait_for_preloader(true),
    )
    .await?
    else {
        // wait_for_preloader returns None when it detects the device already
        // entered fastboot mode — that is success, not an error.
        output::status::ok("[+]", "fastboot mode detected while waiting for preloader");
        fastboot::list_fastboot_devices().await;
        info!(total_secs = start_all.elapsed().as_secs_f32(), sends = 0u64, "force-fastboot complete");
        return Ok(());
    };

    output::status::ok("[+]", format!("{port} appeared"));
    output::status::blank();

    let dev = serial::open_with_permission_recovery(&port)?;
    let start = Instant::now();

    let spinner = output::spinner::start("Waiting for preloader handshake...");

    let count = handshake::handshake(
        dev,
        &port,
        None,
        |event| match event {
            handshake::HandshakeEvent::Write { count } => {
                trace!(sends = count, elapsed = ?start.elapsed(), "writing FASTBOOT");
            }
            handshake::HandshakeEvent::PortLost { port } => {
                output::status::warn("[!]", format!("{port} disconnected"));
                output::status::warn("[!]", format!("{port} lost, waiting for reconnect"));
            }
            handshake::HandshakeEvent::PortReconnected { port } => {
                output::status::ok("[+]", format!("{port} reconnected"));
            }
        },
    )
    .await?;

    output::spinner::succeed(&spinner);

    let elapsed = start.elapsed().as_secs_f32();
    output::status::blank();
    output::status::ok("[+]", format!("fastboot mode detected ({count} writes)"));
    debug!(sends = count, elapsed_secs = elapsed, "handshake succeeded");

    fastboot::list_fastboot_devices().await;

    // On Windows, warn if nusb can't detect the device (missing WinUSB driver).
    // The Tauri GUI has this check; CLI users need the same guidance.
    #[cfg(target_os = "windows")]
    if !fastboot::in_fastboot_mode().await {
        output::status::warn("[!]", "Device left preloader. If it does not appear in fastboot, install the WinUSB driver via Zadig for the fastboot VID:PID.");
    }

    info!(total_secs = start_all.elapsed().as_secs_f32(), sends = count, "force-fastboot complete");
    Ok(())
}

/// Simulated handshake: 5 stages with realistic timing and terminal output.
async fn run_simulated() -> Result<()> {
    output::status::heading("[!] SIMULATED MODE — no device will be touched");
    output::status::blank();

    // ── Stage 1: Check fastboot mode ─────────────────────────────────
    output::status::heading("[1/5] Checking for fastboot mode...");
    sleep(Duration::from_secs(1)).await;
    output::status::dim("  [x] no fastboot device found");
    output::status::blank();

    // ── Stage 2: Wait for preloader serial port ──────────────────────
    output::status::heading("[2/5] Waiting for preloader serial port...");
    let sp = output::spinner::start("Scanning serial ports...");
    for _ in 0..12 {
        sleep(Duration::from_millis(250)).await;
    }
    output::spinner::succeed(&sp);
    output::status::ok("[+]", "/dev/ttyACM0 appeared");
    output::status::blank();

    // ── Stage 3: Send handshake until disconnect ─────────────────────
    output::status::heading("[3/5] Sending FASTBOOT handshake...");
    for n in 1..=5 {
        sleep(Duration::from_millis(500)).await;
        output::status::dim(format!("  -> write #{n}"));
    }
    output::status::warn("[!]", "FASTBOOT write failed — device disconnected");
    output::status::blank();

    // ── Stage 4: Wait for reconnect ──────────────────────────────────
    output::status::heading("[4/5] Waiting for device to reconnect...");
    let sp = output::spinner::start("Scanning serial ports...");
    for _ in 0..8 {
        sleep(Duration::from_millis(250)).await;
    }
    output::spinner::succeed(&sp);
    output::status::ok("[+]", "/dev/ttyACM0 reconnected");
    output::status::blank();

    // ── Stage 5: Continue handshake -> fastboot mode ──────────────────
    output::status::heading("[5/5] Continuing handshake...");
    for n in 6..=10 {
        sleep(Duration::from_millis(500)).await;
        output::status::dim(format!("  -> write #{n}"));
    }
    sleep(Duration::from_millis(500)).await;

    output::status::blank();
    output::status::ok("[+]", "fastboot mode detected (simulated)");
    debug!(sends = 10u64, elapsed_secs = 11.0, "force-fastboot simulated handshake complete");
    info!(total_secs = 11.0_f32, sends = 10u64, "SIM force-fastboot complete");
    Ok(())
}
