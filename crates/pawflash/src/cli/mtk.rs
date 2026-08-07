use std::path::PathBuf;

use clap::Subcommand;
use miette::{miette, Context, IntoDiagnostic, Result};
use tracing::{debug, info};

use pawflash_core::mtk::{
    self, current_platform, current_version, erase_partition, fetch_manifest, install_root,
    read_partition, write_partition, MtkEvent, PartType,
};
use pawflash_core::output;

/// Subcommands for the `pawflash mtkclient` DA-mode tool.
#[derive(Debug, Subcommand)]
pub enum MtkAction {
    /// Show installed bridge version and binary path
    Status,
    /// Download and install the mtk bridge from the release manifest
    Download,
    /// Remove the installed mtk bridge
    Remove {
        /// Skip the confirmation prompt
        #[arg(long)]
        yes: bool,
    },
    /// Check bridge install, device visibility, and platform support
    Doctor,
    /// Read a partition to a file
    Read {
        /// Partition name (e.g. boot, system)
        #[arg(long)]
        partition: String,
        /// Output file path
        #[arg(long)]
        file: PathBuf,
        /// Storage partition type: user, boot1, boot2, or rpmb
        #[arg(long, value_parser = parse_parttype, default_value = "user")]
        parttype: PartType,
    },
    /// Write a file to a partition
    Write {
        /// Partition name (e.g. boot, system)
        #[arg(long)]
        partition: String,
        /// Input file path
        #[arg(long)]
        file: PathBuf,
        /// Storage partition type: user, boot1, boot2, or rpmb
        #[arg(long, value_parser = parse_parttype, default_value = "user")]
        parttype: PartType,
    },
    /// Erase a partition
    Erase {
        /// Partition name (e.g. boot, system)
        #[arg(long)]
        partition: String,
        /// Storage partition type: user, boot1, boot2, or rpmb
        #[arg(long, value_parser = parse_parttype, default_value = "user")]
        parttype: PartType,
    },
}

fn parse_parttype(s: &str) -> std::result::Result<PartType, String> {
    match s {
        "user" => Ok(PartType::User),
        "boot1" => Ok(PartType::Boot1),
        "boot2" => Ok(PartType::Boot2),
        "rpmb" => Ok(PartType::Rpmb),
        _ => Err(format!("invalid parttype '{s}': expected user, boot1, boot2, or rpmb")),
    }
}

/// Run a `pawflash mtkclient` subcommand.
///
/// When `simulate` is true, all device operations run through the simulated
/// runner with no subprocess and no device access.
///
/// # Errors
///
/// Returns an error if the manifest is unreachable, install fails, or the
/// bridge operation fails.
pub fn run(action: MtkAction, simulate: bool) -> Result<()> {
    debug!("mtkclient command: {action:?}");
    if simulate {
        output::status::heading("⚠ SIMULATED MODE — no device will be touched");
    }

    match action {
        MtkAction::Status => {
            run_status(simulate);
            Ok(())
        }
        MtkAction::Download => run_download(simulate),
        MtkAction::Remove { yes } => run_remove(yes),
        MtkAction::Doctor => run_doctor(simulate),
        MtkAction::Read { partition, file, parttype } => {
            run_read(&partition, &file, parttype, simulate)
        }
        MtkAction::Write { partition, file, parttype } => {
            run_write(&partition, &file, parttype, simulate)
        }
        MtkAction::Erase { partition, parttype } => run_erase(&partition, parttype, simulate),
    }
}

fn run_status(simulate: bool) {
    if simulate {
        output::status::data("mtk bridge: not installed (simulated)");
        return;
    }
    if let Some(version) = current_version() {
        output::status::ok("mtk bridge installed", version);
        let bin = install_root().join("bridge").join(bridge_exe());
        output::status::data(format!("binary: {}", bin.display()));
    } else {
        output::status::warn("mtk bridge", "not installed");
        output::status::data("run `pawflash mtkclient download` to install");
    }
}

fn run_download(simulate: bool) -> Result<()> {
    if simulate {
        output::status::data("mtk bridge: download skipped (simulated)");
        return Ok(());
    }

    output::status::data("fetching mtk bridge manifest...");
    let manifest = fetch_manifest().context("failed to fetch mtk bridge manifest")?;
    let platform = current_platform()?;
    let asset = manifest.asset_for(&platform)?;
    let sha_prefix = &asset.sha256[..8.min(asset.sha256.len())];
    output::status::data(format!("platform {platform}: {sha_prefix} ({url})", url = asset.url));
    info!(version = %manifest.version, "installing mtk bridge");

    // Byte/total progress bar during the download phase.
    let pb = output::spinner::partition_progress_bar("download");
    let mut on_progress = |done: u64, total: u64| {
        if total > 0 {
            pb.set_length(total);
        }
        pb.set_position(done);
    };
    let result = mtk::ensure_installed(&manifest, Some(&mut on_progress));
    match result {
        Ok(bin) => {
            pb.finish_and_clear();
            output::status::ok("mtk bridge installed", manifest.version);
            output::status::data(format!("binary: {}", bin.display()));
        }
        Err(e) => {
            pb.abandon();
            info!(error = %e, "mtk bridge install failed");
            return Err(e).context("failed to install mtk bridge");
        }
    }
    Ok(())
}

fn run_remove(yes: bool) -> Result<()> {
    let root = install_root();
    if !root.exists() {
        output::status::warn("mtk bridge", "not installed");
        return Ok(());
    }
    if !yes {
        let proceed = inquire::Confirm::new("Remove the installed mtk bridge?")
            .prompt()
            .into_diagnostic()?;
        if !proceed {
            output::status::data("aborted");
            return Ok(());
        }
    }
    std::fs::remove_dir_all(&root)
        .map_err(|e| miette!("failed to remove {}: {e}", root.display()))?;
    output::status::ok("mtk bridge removed", "");
    Ok(())
}

fn run_doctor(simulate: bool) -> Result<()> {
    output::status::heading("mtk bridge doctor");
    if simulate {
        output::status::ok("platform", current_platform()?);
        output::status::ok("bridge", "not installed (simulated)");
        output::status::ok("device", "not checked (simulated)");
        return Ok(());
    }

    match current_platform() {
        Ok(p) => output::status::ok("platform", p),
        Err(e) => output::status::fail("platform", format!("{e}")),
    }

    match current_version() {
        Some(v) => output::status::ok("bridge", format!("installed ({v})")),
        None => output::status::fail("bridge", "not installed — run `pawflash mtkclient download`"),
    }

    match fetch_manifest() {
        Ok(m) => output::status::ok("manifest", format!("reachable ({})", m.version)),
        Err(e) => output::status::fail("manifest", format!("{e}")),
    }

    #[cfg(target_os = "linux")]
    {
        if pawflash_core::udev::ensure_udev_rules() {
            output::status::ok("udev", "rules installed");
        } else {
            output::status::fail("udev", "rules not installed (run as root or install manually)");
        }
    }

    #[cfg(target_os = "windows")]
    {
        match pawflash_core::mtk::ensure_usbdk() {
            Ok(()) => output::status::ok("usbdk", "present"),
            Err(e) => output::status::fail("usbdk", format!("{e}")),
        }
    }

    output::status::ok("doctor", "checks complete");
    Ok(())
}

/// Forward `on_event` to the console (progress lines).
fn forward_event(ev: &MtkEvent) {
    match ev {
        MtkEvent::Phase { phase, message } => {
            info!(phase = %phase, message = %message, "mtk bridge phase");
            output::status::dim(format!("[{phase}] {message}"));
        }
        MtkEvent::Progress { bytes } => {
            let _ = output::spinner::print(&format!("  {bytes} bytes transferred"));
        }
        MtkEvent::Log { level, message } => {
            debug!(level = %level, message = %message, "mtk bridge log");
        }
        _ => {}
    }
}

/// Resolve the manifest, or a dummy when simulating (no network needed).
fn manifest_for(simulate: bool) -> Result<pawflash_core::mtk::Manifest> {
    if simulate {
        // The simulated runner never inspects the manifest.
        return Ok(pawflash_core::mtk::Manifest {
            version: "simulated".into(),
            commit: String::new(),
            platforms: std::collections::HashMap::new(),
        });
    }
    fetch_manifest().context("failed to fetch mtk bridge manifest")
}

fn run_read(partition: &str, file: &std::path::Path, parttype: PartType, simulate: bool) -> Result<()> {
    let manifest = manifest_for(simulate)?;
    output::status::data(format!("reading {partition} ({parttype}) → {}", file.display()));
    let bytes = read_partition(&manifest, partition, file, parttype, simulate, &mut forward_event)
        .context("mtk read failed")?;
    output::status::ok("read complete", format!("{bytes} bytes"));
    Ok(())
}

fn run_write(partition: &str, file: &std::path::Path, parttype: PartType, simulate: bool) -> Result<()> {
    if !simulate && !file.exists() {
        return Err(miette!("file not found: {}", file.display()));
    }
    let manifest = manifest_for(simulate)?;
    output::status::data(format!("writing {} → {partition} ({parttype})", file.display()));
    let bytes = write_partition(&manifest, partition, file, parttype, simulate, &mut forward_event)
        .context("mtk write failed")?;
    output::status::ok("write complete", format!("{bytes} bytes"));
    Ok(())
}

fn run_erase(partition: &str, parttype: PartType, simulate: bool) -> Result<()> {
    let manifest = manifest_for(simulate)?;
    output::status::data(format!("erasing {partition} ({parttype})"));
    erase_partition(&manifest, partition, parttype, simulate, &mut forward_event)
        .context("mtk erase failed")?;
    output::status::ok("erase complete", "");
    Ok(())
}

const fn bridge_exe() -> &'static str {
    if cfg!(target_os = "windows") { "bridge.exe" } else { "bridge" }
}
