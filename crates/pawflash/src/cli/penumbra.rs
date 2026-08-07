//! CLI handler for `pawflash penumbra`, the native DA-mode backend.
//!
//! Mirrors `cli/mtk.rs` structure: a flat `PenumbraAction` enum for device
//! ops plus a `DaAction` group owning DA acquisition. Ops resolve DA bytes via
//! `--da` override → persisted selection, then route through the penumbra
//! runner (real or simulated).

use std::path::{Path, PathBuf};

use clap::Subcommand;
use miette::{Context, IntoDiagnostic, Result};
use tracing::{debug, info};

use pawflash_core::penumbra::{
    clear_selection, da_cache_path, download_da, fetch_da_manifest, load_selection,
    remove_cached_da, resolve_by_brand_chipset, resolve_by_device, save_selection, verify_da,
    DaSelection, PenumbraBootMode, PenumbraEvent,
};
use pawflash_core::{output, udev};

/// Subcommands for the `pawflash penumbra` native DA-mode tool.
#[derive(Debug, Subcommand)]
pub enum PenumbraAction {
    /// Show penumbra status (DA selection, device visibility, manifest reachability)
    Status,
    /// Check DA selection, manifest, and device prerequisites
    Doctor,
    /// DA lifecycle: download/status/remove
    Da {
        #[command(subcommand)]
        command: DaAction,
    },
    /// SPFT-style flash (locked-bootloader safe)
    Download {
        /// Partition name (e.g. boot, system)
        #[arg(long)]
        partition: String,
        /// Input file path
        #[arg(long)]
        file: PathBuf,
    },
    /// GPT-offset flash via `write_partition`
    Write {
        /// Partition name (e.g. boot, system)
        #[arg(long)]
        partition: String,
        /// Input file path
        #[arg(long)]
        file: PathBuf,
    },
    /// GPT-offset read via `read_partition`
    Read {
        /// Partition name (e.g. boot, system)
        #[arg(long)]
        partition: String,
        /// Output file path
        #[arg(long)]
        file: PathBuf,
    },
    /// SPFT-style readback
    Upload {
        /// Partition name (e.g. boot, system)
        #[arg(long)]
        partition: String,
        /// Output file path
        #[arg(long)]
        file: PathBuf,
    },
    /// Erase a partition
    Erase {
        /// Partition name (e.g. boot, system)
        #[arg(long)]
        partition: String,
    },
    /// Format a partition
    Format {
        /// Partition name (e.g. boot, system)
        #[arg(long)]
        partition: String,
    },
    /// Read flash at an offset
    #[command(name = "read-offset")]
    ReadOffset {
        /// Address (decimal or 0x hex)
        #[arg(long)]
        address: String,
        /// Byte length
        #[arg(long)]
        length: String,
        /// Output file path
        #[arg(long)]
        file: PathBuf,
    },
    /// Write flash at an offset
    #[command(name = "write-offset")]
    WriteOffset {
        /// Address (decimal or 0x hex)
        #[arg(long)]
        address: String,
        /// Input file path
        #[arg(long)]
        file: PathBuf,
    },
    /// Erase flash at an offset
    #[command(name = "erase-offset")]
    EraseOffset {
        /// Address (decimal or 0x hex)
        #[arg(long)]
        address: String,
        /// Byte length
        #[arg(long)]
        length: String,
    },
    /// Read all partitions to a directory
    #[command(name = "read-all")]
    ReadAll {
        /// Output directory
        #[arg(long)]
        dir: PathBuf,
        /// Partitions to skip (comma-separated)
        #[arg(long, value_delimiter = ',')]
        skip: Vec<String>,
    },
    /// Write all partitions from a directory
    #[command(name = "write-all")]
    WriteAll {
        /// Input directory
        #[arg(long)]
        dir: PathBuf,
        /// Partitions to skip (comma-separated)
        #[arg(long, value_delimiter = ',')]
        skip: Vec<String>,
        /// Ignore missing partitions instead of erroring
        #[arg(long)]
        ignore_missing: bool,
    },
    /// Print the partition table
    Pgpt,
    /// Unlock or lock the bootloader (seccfg)
    Seccfg {
        /// unlock or lock
        #[arg(value_parser = ["unlock", "lock"])]
        action: String,
    },
    /// Read memory (DA extensions required)
    Peek {
        /// Address (decimal or 0x hex)
        #[arg(long)]
        address: String,
        /// Byte length
        #[arg(long)]
        length: String,
        /// Output file path
        #[arg(long)]
        file: PathBuf,
    },
    /// Write memory (DA extensions required)
    Poke {
        /// Address (decimal or 0x hex)
        #[arg(long)]
        address: String,
        /// Input file path
        #[arg(long)]
        file: PathBuf,
    },
    /// RPMB operations
    Rpmb {
        #[command(subcommand)]
        command: RpmbAction,
    },
    /// Reboot the device
    Reboot {
        /// normal, homescreen, fastboot, meta, or test
        #[arg(value_parser = ["normal", "homescreen", "fastboot", "meta", "test"], default_value = "normal")]
        mode: String,
    },
    /// Shut the device down through DA mode
    Shutdown,
    /// Set the active boot slot
    #[command(name = "set-slot")]
    SetSlot {
        /// a or b
        #[arg(value_parser = ["a", "b"])]
        slot: String,
    },
    /// Crash the device into bootrom (preloader-only)
    Crash,
}

/// DA lifecycle subcommands.
#[derive(Debug, Subcommand)]
pub enum DaAction {
    /// Download a DA file (interactive device prompt, or --brand/--chipset, or --da <path>)
    Download {
        /// Device model name to search for (e.g. "Infinix NOTE 12")
        device: Option<String>,
        /// Brand (e.g. infinix)
        #[arg(long)]
        brand: Option<String>,
        /// Chipset (e.g. mt6789)
        #[arg(long)]
        chipset: Option<String>,
        /// Use a local DA file directly (skips the manifest)
        #[arg(long = "da")]
        da_path: Option<PathBuf>,
    },
    /// Show the installed DA selection
    Status,
    /// Remove all cached DAs and clear the selection
    Remove {
        /// Skip the confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}

/// RPMB subcommands.
#[derive(Debug, Subcommand)]
pub enum RpmbAction {
    /// Read RPMB sectors to a file
    Read {
        /// RPMB region (0-3)
        #[arg(long, default_value_t = 0)]
        region: u8,
        /// Starting sector
        #[arg(long, default_value_t = 0)]
        start_sector: u32,
        /// Number of sectors
        #[arg(long)]
        sectors: u32,
        /// Output file path
        #[arg(long)]
        file: PathBuf,
    },
    /// Write a file to RPMB sectors
    Write {
        /// RPMB region (0-3)
        #[arg(long, default_value_t = 0)]
        region: u8,
        /// Starting sector
        #[arg(long, default_value_t = 0)]
        start_sector: u32,
        /// Number of sectors
        #[arg(long)]
        sectors: u32,
        /// Input file path
        #[arg(long)]
        file: PathBuf,
    },
    /// Authenticate RPMB with a hex key
    Auth {
        /// RPMB region (0-3)
        #[arg(long, default_value_t = 0)]
        region: u8,
        /// 32-byte hex authentication key
        key: String,
    },
}

/// Run a `pawflash penumbra` subcommand.
///
/// When `simulate` is true, all device operations run through the simulated
/// runner with no device and no DA download.
///
/// # Errors
///
/// Returns an error if the manifest is unreachable, DA resolution fails, or
/// the device operation fails.
pub fn run(action: PenumbraAction, simulate: bool) -> Result<()> {
    debug!("penumbra command: {action:?}");
    if simulate {
        output::status::heading("⚠ SIMULATED MODE — no device will be touched");
    }

    match action {
        PenumbraAction::Status => {
            run_status(simulate);
            Ok(())
        }
        PenumbraAction::Doctor => {
            run_doctor(simulate);
            Ok(())
        }
        PenumbraAction::Da { command } => match command {
            DaAction::Download { device, brand, chipset, da_path } => {
                run_da_download(device.as_deref(), brand.as_deref(), chipset.as_deref(), da_path, simulate)
            }
            DaAction::Status => {
                run_da_status();
                Ok(())
            }
            DaAction::Remove { yes } => run_da_remove(yes),
        },
        PenumbraAction::Download { partition, file } => {
            run_download(&partition, &file, simulate)
        }
        PenumbraAction::Write { partition, file } => run_write(&partition, &file, simulate),
        PenumbraAction::Read { partition, file } => run_read(&partition, &file, simulate),
        PenumbraAction::Upload { partition, file } => run_upload(&partition, &file, simulate),
        PenumbraAction::Erase { partition } => run_erase(&partition, simulate),
        PenumbraAction::Format { partition } => run_format(&partition, simulate),
        PenumbraAction::ReadOffset { address, length, file } => {
            run_read_offset(&address, &length, &file, simulate)
        }
        PenumbraAction::WriteOffset { address, file } => run_write_offset(&address, &file, simulate),
        PenumbraAction::EraseOffset { address, length } => {
            run_erase_offset(&address, &length, simulate)
        }
        PenumbraAction::ReadAll { dir, skip } => run_read_all(&dir, &skip, simulate),
        PenumbraAction::WriteAll { dir, skip, ignore_missing } => {
            run_write_all(&dir, &skip, ignore_missing, simulate)
        }
        PenumbraAction::Pgpt => run_pgpt(simulate),
        PenumbraAction::Seccfg { action } => run_seccfg(&action, simulate),
        PenumbraAction::Peek { address, length, file } => {
            run_peek(&address, &length, &file, simulate)
        }
        PenumbraAction::Poke { address, file } => run_poke(&address, &file, simulate),
        PenumbraAction::Rpmb { command } => run_rpmb(command, simulate),
        PenumbraAction::Reboot { mode } => run_reboot(&mode, simulate),
        PenumbraAction::Shutdown => run_shutdown(simulate),
        PenumbraAction::SetSlot { slot } => run_set_slot(&slot, simulate),
        PenumbraAction::Crash => run_crash(simulate),
    }
}

fn run_download(partition: &str, file: &Path, simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    pawflash_core::penumbra::download_flash(&da, partition, file, simulate, &mut ev)
        .context("penumbra download failed")?;
    output::status::ok("download complete", partition);
    Ok(())
}

fn run_write(partition: &str, file: &Path, simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    let bytes = pawflash_core::penumbra::write_partition(&da, partition, file, simulate, &mut ev)
        .context("penumbra write failed")?;
    output::status::ok("write complete", format!("{bytes} bytes"));
    Ok(())
}

fn run_read(partition: &str, file: &Path, simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    let bytes = pawflash_core::penumbra::read_partition(&da, partition, file, simulate, &mut ev)
        .context("penumbra read failed")?;
    output::status::ok("read complete", format!("{bytes} bytes"));
    Ok(())
}

fn run_upload(partition: &str, file: &Path, simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    pawflash_core::penumbra::upload(&da, partition, file, simulate, &mut ev)
        .context("penumbra upload failed")?;
    output::status::ok("upload complete", partition);
    Ok(())
}

fn run_erase(partition: &str, simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    pawflash_core::penumbra::erase_partition(&da, partition, simulate, &mut ev)
        .context("penumbra erase failed")?;
    output::status::ok("erase complete", partition);
    Ok(())
}

fn run_format(partition: &str, simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    pawflash_core::penumbra::format(&da, partition, simulate, &mut ev)
        .context("penumbra format failed")?;
    output::status::ok("format complete", partition);
    Ok(())
}

fn run_read_offset(address: &str, length: &str, file: &Path, simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let addr = parse_num(address)?;
    let len = parse_num(length)?;
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    pawflash_core::penumbra::read_offset(&da, addr, len, file, simulate, &mut ev)
        .context("penumbra read-offset failed")?;
    output::status::ok("read-offset complete", format!("{len} bytes"));
    Ok(())
}

fn run_write_offset(address: &str, file: &Path, simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let addr = parse_num(address)?;
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    let section = pawflash_core::penumbra::PartitionKind::Unknown;
    pawflash_core::penumbra::write_offset(&da, addr, section, file, simulate, &mut ev)
        .context("penumbra write-offset failed")?;
    output::status::ok("write-offset complete", "");
    Ok(())
}

fn run_erase_offset(address: &str, length: &str, simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let addr = parse_num(address)?;
    let len = parse_num(length)?;
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    pawflash_core::penumbra::erase_offset(&da, addr, len, simulate, &mut ev)
        .context("penumbra erase-offset failed")?;
    output::status::ok("erase-offset complete", "");
    Ok(())
}

fn run_read_all(dir: &Path, skip: &[String], simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    pawflash_core::penumbra::read_all(&da, dir, skip, simulate, &mut ev)
        .context("penumbra read-all failed")?;
    output::status::ok("read-all complete", dir.display().to_string());
    Ok(())
}

fn run_write_all(dir: &Path, skip: &[String], ignore_missing: bool, simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    pawflash_core::penumbra::write_all(&da, dir, skip, ignore_missing, simulate, &mut ev)
        .context("penumbra write-all failed")?;
    output::status::ok("write-all complete", dir.display().to_string());
    Ok(())
}

fn run_pgpt(simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    let entries = pawflash_core::penumbra::pgpt(&da, simulate, &mut ev)
        .context("penumbra pgpt failed")?;
    for p in &entries {
        output::status::data(format!(
            "{:<24} 0x{addr:016X} 0x{size:016X}  {section}",
            p.name,
            addr = p.address,
            size = p.size,
            section = p.section
        ));
    }
    Ok(())
}

fn run_seccfg(action: &str, simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let unlock = action == "unlock";
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    pawflash_core::penumbra::seccfg(&da, unlock, simulate, &mut ev)
        .context("penumbra seccfg failed")?;
    output::status::ok("seccfg", action);
    Ok(())
}

fn run_peek(address: &str, length: &str, file: &Path, simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let addr = parse_num::<u32>(address)?;
    let len = parse_num(length)?;
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    pawflash_core::penumbra::peek(&da, addr, len, file, simulate, &mut ev)
        .context("penumbra peek failed")?;
    output::status::ok("peek complete", format!("{len} bytes"));
    Ok(())
}

fn run_poke(address: &str, file: &Path, simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let addr = parse_num::<u32>(address)?;
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    pawflash_core::penumbra::poke(&da, addr, file, simulate, &mut ev)
        .context("penumbra poke failed")?;
    output::status::ok("poke complete", "");
    Ok(())
}

fn run_rpmb(command: RpmbAction, simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    match command {
        RpmbAction::Read { region, start_sector, sectors, file } => {
            pawflash_core::penumbra::rpmb_read(&da, region, start_sector, sectors, &file, simulate, &mut ev)
                .context("penumbra rpmb read failed")?;
        }
        RpmbAction::Write { region, start_sector, sectors, file } => {
            pawflash_core::penumbra::rpmb_write(&da, region, start_sector, sectors, &file, simulate, &mut ev)
                .context("penumbra rpmb write failed")?;
        }
        RpmbAction::Auth { region, key } => {
            pawflash_core::penumbra::rpmb_auth(&da, region, &key, simulate, &mut ev)
                .context("penumbra rpmb auth failed")?;
        }
    }
    output::status::ok("rpmb", "complete");
    Ok(())
}

fn run_reboot(mode: &str, simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let boot = match mode {
        "normal" => PenumbraBootMode::Normal,
        "homescreen" => PenumbraBootMode::HomeScreen,
        "fastboot" => PenumbraBootMode::Fastboot,
        "meta" => PenumbraBootMode::Meta,
        "test" => PenumbraBootMode::Test,
        _ => unreachable!("clap value_parser restricts mode"),
    };
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    pawflash_core::penumbra::reboot(&da, boot, simulate, &mut ev)
        .context("penumbra reboot failed")?;
    output::status::ok("reboot", mode);
    Ok(())
}

fn run_shutdown(simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    pawflash_core::penumbra::shutdown(&da, simulate, &mut ev).context("penumbra shutdown failed")?;
    output::status::ok("shutdown", "command sent");
    Ok(())
}

fn run_set_slot(slot: &str, simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let slot_a = slot == "a";
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    pawflash_core::penumbra::set_active_slot(&da, slot_a, simulate, &mut ev)
        .context("penumbra set-slot failed")?;
    output::status::ok("set-slot", slot);
    Ok(())
}

fn run_crash(simulate: bool) -> Result<()> {
    let da = resolve_da(simulate)?;
    let mut ev = |e: &PenumbraEvent| forward_event(e);
    pawflash_core::penumbra::crash(&da, simulate, &mut ev).context("penumbra crash failed")?;
    output::status::ok("crash", "device should be in bootrom");
    Ok(())
}

fn run_status(simulate: bool) {
    if simulate {
        output::status::data("penumbra: not installed (simulated)");
        return;
    }
    if let Some(sel) = load_selection() {
        output::status::ok("DA selected", format!("{} ({})", sel.brand, sel.chipset));
        output::status::data(format!("path: {}", sel.path));
    } else {
        output::status::warn("DA", "none selected");
        output::status::data("run `pawflash penumbra da download` to pick a DA");
    }
    let visible = device_visible_blocking();
    if visible {
        output::status::ok("device", "visible");
    } else {
        output::status::warn("device", "not visible (is it in BROM/preloader/DA mode?)");
    }
}

fn run_doctor(simulate: bool) {
    output::status::heading("penumbra doctor");
    if simulate {
        output::status::ok("DA", "none selected (simulated)");
        output::status::ok("manifest", "not checked (simulated)");
        output::status::ok("device", "not checked (simulated)");
        return;
    }

    match load_selection() {
        Some(sel) => output::status::ok("DA", format!("{} ({})", sel.brand, sel.chipset)),
        None => output::status::fail("DA", "none selected — run `pawflash penumbra da download`"),
    }

    match fetch_da_manifest() {
        Ok(m) => output::status::ok("manifest", format!("reachable ({})", m.version)),
        Err(e) => output::status::fail("manifest", format!("{e}")),
    }

    if device_visible_blocking() {
        output::status::ok("device", "visible");
    } else {
        output::status::fail("device", "not visible (is it in BROM/preloader/DA mode?)");
    }

    #[cfg(target_os = "windows")]
    output::status::data("hint: install a WinUSB driver via Zadig for the MTK device");

    output::status::ok("doctor", "checks complete");
}

fn run_da_download(
    device: Option<&str>,
    brand: Option<&str>,
    chipset: Option<&str>,
    da_path: Option<PathBuf>,
    simulate: bool,
) -> Result<()> {
    if simulate {
        output::status::data("penumbra da download: skipped (simulated)");
        return Ok(());
    }

    // Explicit local DA file: verify + save selection, no manifest.
    if let Some(path) = da_path {
        if !path.exists() {
            return Err(miette::miette!("file not found: {}", path.display()));
        }
        let sel = DaSelection {
            brand: "local".into(),
            chipset: "custom".into(),
            path: path.display().to_string(),
            sha256: String::new(),
        };
        save_selection(&sel).into_diagnostic()?;
        output::status::ok("DA selected", path.display().to_string());
        return Ok(());
    }

    let entry = if let (Some(b), Some(c)) = (brand, chipset) {
        resolve_by_brand_chipset(b, c)
    } else {
        resolve_by_device(device.unwrap_or(""))
    }
    .context("failed to resolve DA")?;

    let pb = output::spinner::partition_progress_bar("da");
    let mut on_progress = |done: u64, total: u64| {
        if total > 0 {
            pb.set_length(total);
        }
        pb.set_position(done);
    };
    let path = download_da(&entry, &mut on_progress).context("failed to download DA")?;
    pb.finish_and_clear();

    let sel = DaSelection {
        brand: entry.brand.clone(),
        chipset: entry.chipset.clone(),
        path: path.display().to_string(),
        sha256: entry.sha256.clone(),
    };
    save_selection(&sel).into_diagnostic()?;
    output::status::ok("DA installed", format!("{} ({})", entry.brand, entry.chipset));
    output::status::data(format!("path: {}", path.display()));
    Ok(())
}

fn run_da_status() {
    if let Some(sel) = load_selection() {
        output::status::ok("DA selected", format!("{} ({})", sel.brand, sel.chipset));
        output::status::data(format!("path: {}", sel.path));
        if !sel.sha256.is_empty() {
            output::status::data(format!("sha256: {}", &sel.sha256[..8.min(sel.sha256.len())]));
        }
        let path = std::path::Path::new(&sel.path);
        if path.exists() {
            match pawflash_core::penumbra::parse_da_file(path) {
                Ok(da) => {
                    let codes: Vec<String> =
                        da.das.iter().map(|d| format!("0x{:04X}", d.hw_code)).collect();
                    output::status::data(format!("supported hw_codes: {}", codes.join(", ")));
                }
                Err(e) => output::status::warn("parse", format!("{e}")),
            }
        }
    } else {
        output::status::warn("DA", "none selected");
        output::status::data("run `pawflash penumbra da download` to pick a DA");
    }
}

fn run_da_remove(yes: bool) -> Result<()> {
    if load_selection().is_none() && !da_cache_path("", "").exists() {
        output::status::warn("DA", "none installed");
        return Ok(());
    }
    if !yes {
        let proceed = inquire::Confirm::new("Remove all cached DAs and the selection?")
            .prompt()
            .into_diagnostic()?;
        if !proceed {
            output::status::data("aborted");
            return Ok(());
        }
    }
    remove_cached_da().into_diagnostic()?;
    clear_selection().into_diagnostic()?;
    output::status::ok("DA cache", "removed");
    Ok(())
}

/// Resolve DA bytes: `--da` override → persisted selection → error hint.
///
/// `simulate` returns an empty slice (simulated runs need no DA).
fn resolve_da(simulate: bool) -> Result<Vec<u8>> {
    if simulate {
        return Ok(Vec::new());
    }
    let sel = load_selection().ok_or_else(|| miette::miette!("no DA selected"))?;
    let bytes = std::fs::read(&sel.path)
        .into_diagnostic()
        .wrap_err_with(|| format!("failed to read DA at {}", sel.path))?;
    if !sel.sha256.is_empty() {
        verify_da(std::path::Path::new(&sel.path), &sel.sha256)
            .context("cached DA failed verification — rerun `pawflash penumbra da download`")?;
    }
    Ok(bytes)
}

/// Parse a number argument (decimal or `0x` hex) into `T`.
fn parse_num<T>(s: &str) -> Result<T>
where
    T: TryFrom<u64>,
    T::Error: std::fmt::Display,
{
    let value: u64 = if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).map_err(|_| miette::miette!("invalid hex number: {s}"))?
    } else {
        s.parse().map_err(|_| miette::miette!("invalid number: {s}"))?
    };
    T::try_from(value).map_err(|e| miette::miette!("number out of range for target type: {e}"))
}

/// Check whether an MTK device is visible (wraps the async nusb probe in a
/// short-lived current-thread runtime).
fn device_visible_blocking() -> bool {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .is_ok_and(|rt| rt.block_on(udev::device_visible()))
}

/// Forward `on_event` to the console (progress lines).
fn forward_event(ev: &PenumbraEvent) {
    match ev {
        PenumbraEvent::Phase { phase, message } => {
            info!(phase = %phase, message = %message, "penumbra phase");
            output::status::dim(format!("[{phase}] {message}"));
        }
        PenumbraEvent::Progress { bytes, total } => {
            let _ = output::spinner::print(&format!("  {bytes} bytes / {total}"));
        }
        PenumbraEvent::Log { level, message } => {
            debug!(level = %level, message = %message, "penumbra log");
        }
        PenumbraEvent::Done { ok, detail } => {
            if *ok {
                output::status::ok("done", detail);
            } else {
                output::status::fail("done", detail);
            }
        }
    }
}
