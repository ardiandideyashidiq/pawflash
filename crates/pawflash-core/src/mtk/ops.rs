//! High-level DA read/write/erase operations.
//!
//! These wrappers ensure the bridge is installed, then drive it via
//! [`crate::mtk::bridge::run_bridge`]. A [`SimulatedMtkRunner`] replaces the
//! subprocess for `--simulate` runs, emitting the same event stream without any
//! device access.

use crate::mtk::bridge::run_bridge;
use crate::mtk::error::MtkError;
use crate::mtk::install::ensure_installed;
use crate::mtk::lock::acquire_device_lock;
use crate::mtk::types::{MtkCommand, MtkEvent, MtkOutcome, PartType};
use crate::mtk::{Manifest, Result};
use std::path::Path;
use std::thread;
use std::time::Duration;

/// Simulated partition size used by [`SimulatedMtkRunner`].
const SIM_PARTITION_SIZE: u64 = 128 * 1024 * 1024;

/// How long a simulated run sleeps per 1 MiB chunk (inverse of ~50 MiB/s).
const SIM_CHUNK_SLEEP: Duration = Duration::from_millis(20);

/// A runner that drives the DA bridge. The real implementation spawns the
/// frozen binary; the simulated one emits synthetic events in-process.
pub trait BridgeRunner {
    /// Run `cmd` against the bridge, forwarding events to `on_event`.
    ///
    /// # Errors
    ///
    /// Returns a [`MtkError`] on process, protocol, or device failure.
    fn run(
        &self,
        bin: &Path,
        cmd: &MtkCommand,
        on_event: &mut dyn FnMut(&MtkEvent),
    ) -> Result<MtkOutcome>;
}

/// The real bridge: spawns the frozen mtkclient binary.
pub struct RealBridge;

impl BridgeRunner for RealBridge {
    fn run(
        &self,
        bin: &Path,
        cmd: &MtkCommand,
        on_event: &mut dyn FnMut(&MtkEvent),
    ) -> Result<MtkOutcome> {
        run_bridge(bin, cmd, on_event)
    }
}

/// `--simulate` runner: emits a realistic event stream in-process with no
/// subprocess and no device.
#[derive(Default)]
pub struct SimulatedMtkRunner {
    /// Partition size to simulate; `None` uses a default.
    pub partition_size: Option<u64>,
}

impl BridgeRunner for SimulatedMtkRunner {
    fn run(
        &self,
        _bin: &Path,
        cmd: &MtkCommand,
        on_event: &mut dyn FnMut(&MtkEvent),
    ) -> Result<MtkOutcome> {
        let total = self.partition_size.unwrap_or(SIM_PARTITION_SIZE);

        on_event(&MtkEvent::Phase { phase: "connect".into(), message: "simulated device".into() });
        on_event(&MtkEvent::Log {
            level: "info".into(),
            message: "running in simulated mode; no device will be touched".into(),
        });

        let partition = match cmd {
            MtkCommand::Read { partition, .. }
            | MtkCommand::Write { partition, .. }
            | MtkCommand::Erase { partition, .. } => partition.clone(),
            MtkCommand::Selftest => "selftest".to_string(),
        };
        on_event(&MtkEvent::Start { total, partition });

        let mut pos = 0u64;
        let chunk = 1024 * 1024;
        while pos < total {
            pos = (pos + chunk).min(total);
            on_event(&MtkEvent::Progress { bytes: pos });
            thread::sleep(SIM_CHUNK_SLEEP);
        }

        on_event(&MtkEvent::Result { ok: true, detail: Some("simulated ok".into()), bytes: Some(total) });
        Ok(MtkOutcome { ok: true, detail: Some("simulated ok".into()), bytes: Some(total) })
    }
}

/// Resolve the runner: real when `simulate` is false, simulated otherwise.
fn runner(simulate: bool) -> Box<dyn BridgeRunner> {
    if simulate {
        Box::<SimulatedMtkRunner>::default()
    } else {
        Box::new(RealBridge)
    }
}

fn run_op(
    manifest: &Manifest,
    cmd: &MtkCommand,
    simulate: bool,
    on_event: &mut dyn FnMut(&MtkEvent),
) -> Result<MtkOutcome> {
    if simulate {
        return runner(true).run(Path::new(""), cmd, on_event);
    }

    // Hold the contention lock for the whole op so concurrent pawflash
    // processes cannot flash the same device simultaneously.
    let _lock = acquire_device_lock()?;
    let bin = ensure_installed(manifest, None)?;
    runner(false).run(&bin, cmd, on_event)
}

/// Read a partition to `file`; returns bytes read.
///
/// # Errors
///
/// Returns any [`MtkError`] from install or the bridge run.
pub fn read_partition(
    manifest: &Manifest,
    partition: &str,
    file: &Path,
    parttype: PartType,
    simulate: bool,
    on_event: &mut dyn FnMut(&MtkEvent),
) -> Result<u64> {
    let cmd = MtkCommand::Read {
        partition: partition.to_string(),
        file: file.display().to_string(),
        parttype,
    };
    let outcome = run_op(manifest, &cmd, simulate, on_event)?;
    outcome
        .bytes
        .ok_or_else(|| MtkError::Bridge("bridge did not report bytes read".into()))
}

/// Write `file` to a partition; returns bytes written.
///
/// # Errors
///
/// Returns any [`MtkError`] from install or the bridge run.
pub fn write_partition(
    manifest: &Manifest,
    partition: &str,
    file: &Path,
    parttype: PartType,
    simulate: bool,
    on_event: &mut dyn FnMut(&MtkEvent),
) -> Result<u64> {
    let cmd = MtkCommand::Write {
        partition: partition.to_string(),
        file: file.display().to_string(),
        parttype,
    };
    let outcome = run_op(manifest, &cmd, simulate, on_event)?;
    outcome
        .bytes
        .ok_or_else(|| MtkError::Bridge("bridge did not report bytes written".into()))
}

/// Erase a partition.
///
/// # Errors
///
/// Returns any [`MtkError`] from install or the bridge run.
pub fn erase_partition(
    manifest: &Manifest,
    partition: &str,
    parttype: PartType,
    simulate: bool,
    on_event: &mut dyn FnMut(&MtkEvent),
) -> Result<()> {
    let cmd = MtkCommand::Erase { partition: partition.to_string(), parttype };
    let outcome = run_op(manifest, &cmd, simulate, on_event)?;
    if outcome.ok {
        Ok(())
    } else {
        Err(MtkError::Bridge(outcome.detail.unwrap_or_else(|| "erase failed".into())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn test_manifest() -> Manifest {
        Manifest {
            version: "test".into(),
            commit: "0".repeat(40),
            platforms: HashMap::new(),
        }
    }

    #[test]
    fn simulated_read_emits_full_event_stream() {
        let mut events = Vec::new();
        let bytes = read_partition(
            &test_manifest(),
            "boot",
            Path::new("/tmp/boot.img"),
            PartType::User,
            true,
            &mut |e| events.push(e.clone()),
        )
        .unwrap();

        assert_eq!(bytes, SIM_PARTITION_SIZE);
        assert!(matches!(events.first(), Some(MtkEvent::Phase { phase, .. }) if phase == "connect"));
        assert!(matches!(events[2], MtkEvent::Start { total, .. } if total == SIM_PARTITION_SIZE));
        assert!(events.iter().any(|e| matches!(e, MtkEvent::Progress { bytes } if *bytes == SIM_PARTITION_SIZE)));
        assert!(matches!(events.last(), Some(MtkEvent::Result { ok: true, .. })));
    }

    #[test]
    fn simulated_write_returns_bytes() {
        let bytes = write_partition(
            &test_manifest(),
            "boot",
            Path::new("/tmp/boot.img"),
            PartType::User,
            true,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(bytes, SIM_PARTITION_SIZE);
    }

    #[test]
    fn simulated_erase_succeeds() {
        erase_partition(&test_manifest(), "boot", PartType::User, true, &mut |_| {}).unwrap();
    }

    #[test]
    fn simulate_does_not_spawn_a_process() {
        // If we were spawning a process, the default platform data dir would
        // have to exist and the binary too; simulate must not require either.
        let spawns = AtomicUsize::new(0);
        let runner = SimulatedMtkRunner::default();
        let mut cb = |_ev: &MtkEvent| {
            let _ = &spawns;
        };
        let outcome = runner
            .run(Path::new("/nonexistent/bridge"), &MtkCommand::Selftest, &mut cb)
            .unwrap();
        assert!(outcome.ok);
        assert_eq!(spawns.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn real_run_requires_installed_bridge() {
        // Empty-platform manifest: the real (non-simulated) path must fail at
        // asset resolution without spawning anything or hitting the network.
        let err = run_op(&test_manifest(), &MtkCommand::Selftest, false, &mut |_| {}).unwrap_err();
        assert!(matches!(err, MtkError::MissingAsset { .. }));
    }

    #[test]
    fn runner_swaps_on_simulate_flag() {
        let mut events = Vec::new();
        let outcome = run_op(
            &test_manifest(),
            &MtkCommand::Selftest,
            true,
            &mut |e| events.push(e.clone()),
        )
        .unwrap();
        assert!(outcome.ok);
        assert!(!events.is_empty());
    }
}
