//! High-level DA operations via the penumbra library.
//!
//! A [`PenumbraRunner`] abstracts real vs simulated execution. [`RealPenumbra`]
//! opens the device through [`crate::penumbra::device::open_device`]; the
//! [`SimulatedPenumbra`] runner emits the same progress stream in-process with
//! no device and no DA. Free functions (e.g. [`read_partition`]) select the
//! runner based on the `simulate` flag and forward events via `on_event`.
//!
//! Event callbacks are `Send` because penumbra's native progress closure
//! requires `F: FnMut(usize, usize) + Send`.

pub mod real;
pub mod simulate;

pub use real::RealPenumbra;
pub use simulate::SimulatedPenumbra;

use crate::penumbra::types::PenumbraEvent;
use crate::penumbra::Result;
use penumbra::core::storage::PartitionKind;
use std::path::Path;

/// Progress reported per ~1 MiB of transfer.
const PROGRESS_THRESHOLD: u64 = 1024 * 1024;

/// A `Send` event callback.
pub type EventCb<'a> = &'a mut (dyn FnMut(&PenumbraEvent) + Send);

/// Partition table entry as reported by `pgpt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionEntry {
    pub name: String,
    pub address: u64,
    pub size: u64,
    pub section: String,
}

/// Boot mode selector (mirrors penumbra's `BootMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PenumbraBootMode {
    Normal,
    HomeScreen,
    Fastboot,
    Meta,
    Test,
}

impl From<PenumbraBootMode> for penumbra::da::protocol::BootMode {
    fn from(mode: PenumbraBootMode) -> Self {
        match mode {
            PenumbraBootMode::Normal => Self::Normal,
            PenumbraBootMode::HomeScreen => Self::HomeScreen,
            PenumbraBootMode::Fastboot => Self::Fastboot,
            PenumbraBootMode::Meta => Self::Meta,
            PenumbraBootMode::Test => Self::Test,
        }
    }
}

/// A runner driving penumbra operations. Real opens the device; simulated
/// emits synthetic progress in-process.
pub trait PenumbraRunner {
    /// Read a partition to `file`; returns bytes read.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn read_partition(&self, partition: &str, file: &Path, on_event: EventCb<'_>) -> Result<u64>;

    /// Write `file` to a partition; returns bytes written.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn write_partition(&self, partition: &str, file: &Path, on_event: EventCb<'_>) -> Result<u64>;

    /// Erase a partition.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn erase_partition(&self, partition: &str, on_event: EventCb<'_>) -> Result<()>;

    /// SPFT-style flash (locked-bootloader safe).
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn download_flash(&self, partition: &str, file: &Path, on_event: EventCb<'_>) -> Result<()>;

    /// SPFT-style readback.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn upload(&self, partition: &str, file: &Path, on_event: EventCb<'_>) -> Result<()>;

    /// Format a partition.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn format(&self, partition: &str, on_event: EventCb<'_>) -> Result<()>;

    /// Read `length` bytes from `address`.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn read_offset(
        &self,
        address: u64,
        length: usize,
        file: &Path,
        on_event: EventCb<'_>,
    ) -> Result<()>;

    /// Write `file` to `address` with the given section kind.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn write_offset(
        &self,
        address: u64,
        section: PartitionKind,
        file: &Path,
        on_event: EventCb<'_>,
    ) -> Result<()>;

    /// Erase `length` bytes at `address`.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn erase_offset(&self, address: u64, length: usize, on_event: EventCb<'_>) -> Result<()>;

    /// Read all partitions to `dir`, skipping `skip`.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn read_all(&self, dir: &Path, skip: &[String], on_event: EventCb<'_>) -> Result<()>;

    /// Write all partitions from `dir`, skipping `skip`.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn write_all(
        &self,
        dir: &Path,
        skip: &[String],
        ignore_missing: bool,
        on_event: EventCb<'_>,
    ) -> Result<()>;

    /// List the partition table.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn pgpt(&self, on_event: EventCb<'_>) -> Result<Vec<PartitionEntry>>;

    /// Unlock or lock the bootloader (seccfg).
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn seccfg(&self, unlock: bool, on_event: EventCb<'_>) -> Result<()>;

    /// Read memory at `address` into `file`.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn peek(
        &self,
        address: u32,
        length: usize,
        file: &Path,
        on_event: EventCb<'_>,
    ) -> Result<()>;

    /// Write `file` to memory at `address`.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn poke(&self, address: u32, file: &Path, on_event: EventCb<'_>) -> Result<()>;

    /// Read `sectors` from RPMB `region`.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn rpmb_read(
        &self,
        region: u8,
        start_sector: u32,
        sectors: u32,
        file: &Path,
        on_event: EventCb<'_>,
    ) -> Result<()>;

    /// Write `sectors` to RPMB `region`.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn rpmb_write(
        &self,
        region: u8,
        start_sector: u32,
        sectors: u32,
        file: &Path,
        on_event: EventCb<'_>,
    ) -> Result<()>;

    /// Authenticate RPMB with `key_hex` (hex string).
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn rpmb_auth(&self, region: u8, key_hex: &str, on_event: EventCb<'_>) -> Result<()>;

    /// Reboot the device into `mode`.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn reboot(&self, mode: PenumbraBootMode, on_event: EventCb<'_>) -> Result<()>;

    /// Shut the device down through DA mode.
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn shutdown(&self, on_event: EventCb<'_>) -> Result<()>;

    /// Set the active boot slot (`a` when `slot_a` is true).
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn set_active_slot(&self, slot_a: bool, on_event: EventCb<'_>) -> Result<()>;

    /// Crash the device into bootrom (preloader-only; refuses in DA mode).
    ///
    /// # Errors
    ///
    /// Returns a [`crate::penumbra::PenumbraError`] on device open or run failure.
    fn crash(&self, on_event: EventCb<'_>) -> Result<()>;
}

/// Emit a phase event with the given message.
pub(crate) fn emit_phase(on_event: EventCb<'_>, phase: &str, message: &str) {
    on_event(&PenumbraEvent::Phase {
        phase: phase.to_string(),
        message: message.to_string(),
    });
}

/// Emit a done event.
pub(crate) fn emit_done(on_event: EventCb<'_>, ok: bool, detail: String) {
    on_event(&PenumbraEvent::Done { ok, detail });
}

/// Build a `Send` progress callback translating the native `(done, total)`
/// callback into [`PenumbraEvent::Progress`], throttled to ~1 MiB.
pub(crate) fn throttled_progress(
    on_event: EventCb<'_>,
    total: u64,
) -> impl FnMut(usize, usize) + Send + '_ {
    let mut last = 0u64;
    move |done: usize, _| {
        let done = done as u64;
        if done - last >= PROGRESS_THRESHOLD || done >= total {
            last = done;
            on_event(&PenumbraEvent::Progress { bytes: done, total });
        }
    }
}

/// Resolve the runner: real when `simulate` is false, simulated otherwise.
fn runner(simulate: bool, da_bytes: &[u8]) -> Box<dyn PenumbraRunner> {
    if simulate {
        Box::new(SimulatedPenumbra)
    } else {
        Box::new(RealPenumbra::new(da_bytes.to_vec()))
    }
}

fn run_with<T>(
    simulate: bool,
    da_bytes: &[u8],
    op: impl FnOnce(&dyn PenumbraRunner) -> Result<T>,
) -> Result<T> {
    op(&*runner(simulate, da_bytes))
}

/// Read a partition to `file`; returns bytes read.
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn read_partition(
    da_bytes: &[u8],
    partition: &str,
    file: &Path,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<u64> {
    run_with(simulate, da_bytes, |r| r.read_partition(partition, file, on_event))
}

/// Write `file` to a partition; returns bytes written.
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn write_partition(
    da_bytes: &[u8],
    partition: &str,
    file: &Path,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<u64> {
    run_with(simulate, da_bytes, |r| r.write_partition(partition, file, on_event))
}

/// Erase a partition.
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn erase_partition(
    da_bytes: &[u8],
    partition: &str,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.erase_partition(partition, on_event))
}

/// SPFT-style flash (locked-bootloader safe).
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn download_flash(
    da_bytes: &[u8],
    partition: &str,
    file: &Path,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.download_flash(partition, file, on_event))
}

/// SPFT-style readback.
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn upload(
    da_bytes: &[u8],
    partition: &str,
    file: &Path,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.upload(partition, file, on_event))
}

/// Format a partition.
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn format(
    da_bytes: &[u8],
    partition: &str,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.format(partition, on_event))
}

/// Read `length` bytes from `address`.
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn read_offset(
    da_bytes: &[u8],
    address: u64,
    length: usize,
    file: &Path,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.read_offset(address, length, file, on_event))
}

/// Write `file` to `address` with the given section kind.
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn write_offset(
    da_bytes: &[u8],
    address: u64,
    section: PartitionKind,
    file: &Path,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.write_offset(address, section, file, on_event))
}

/// Erase `length` bytes at `address`.
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn erase_offset(
    da_bytes: &[u8],
    address: u64,
    length: usize,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.erase_offset(address, length, on_event))
}

/// Read all partitions to `dir`, skipping `skip`.
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn read_all(
    da_bytes: &[u8],
    dir: &Path,
    skip: &[String],
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.read_all(dir, skip, on_event))
}

/// Write all partitions from `dir`, skipping `skip`.
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn write_all(
    da_bytes: &[u8],
    dir: &Path,
    skip: &[String],
    ignore_missing: bool,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.write_all(dir, skip, ignore_missing, on_event))
}

/// List the partition table.
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn pgpt(da_bytes: &[u8], simulate: bool, on_event: EventCb<'_>) -> Result<Vec<PartitionEntry>> {
    run_with(simulate, da_bytes, |r| r.pgpt(on_event))
}

/// Unlock or lock the bootloader (seccfg).
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn seccfg(
    da_bytes: &[u8],
    unlock: bool,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.seccfg(unlock, on_event))
}

/// Read memory at `address` into `file`.
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn peek(
    da_bytes: &[u8],
    address: u32,
    length: usize,
    file: &Path,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.peek(address, length, file, on_event))
}

/// Write `file` to memory at `address`.
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn poke(
    da_bytes: &[u8],
    address: u32,
    file: &Path,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.poke(address, file, on_event))
}

/// Read `sectors` from RPMB `region`.
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn rpmb_read(
    da_bytes: &[u8],
    region: u8,
    start_sector: u32,
    sectors: u32,
    file: &Path,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.rpmb_read(region, start_sector, sectors, file, on_event))
}

/// Write `sectors` to RPMB `region`.
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn rpmb_write(
    da_bytes: &[u8],
    region: u8,
    start_sector: u32,
    sectors: u32,
    file: &Path,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.rpmb_write(region, start_sector, sectors, file, on_event))
}

/// Authenticate RPMB with `key_hex` (hex string).
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn rpmb_auth(
    da_bytes: &[u8],
    region: u8,
    key_hex: &str,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.rpmb_auth(region, key_hex, on_event))
}

/// Reboot the device into `mode`.
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn reboot(
    da_bytes: &[u8],
    mode: PenumbraBootMode,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.reboot(mode, on_event))
}

/// Shut the device down through DA mode.
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn shutdown(da_bytes: &[u8], simulate: bool, on_event: EventCb<'_>) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.shutdown(on_event))
}

/// Set the active boot slot (`a` when `slot_a` is true).
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn set_active_slot(
    da_bytes: &[u8],
    slot_a: bool,
    simulate: bool,
    on_event: EventCb<'_>,
) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.set_active_slot(slot_a, on_event))
}

/// Crash the device into bootrom (preloader-only).
///
/// `da_bytes` is ignored in simulate mode.
///
/// # Errors
///
/// Returns any [`crate::penumbra::PenumbraError`] from device open or the run.
pub fn crash(da_bytes: &[u8], simulate: bool, on_event: EventCb<'_>) -> Result<()> {
    run_with(simulate, da_bytes, |r| r.crash(on_event))
}
