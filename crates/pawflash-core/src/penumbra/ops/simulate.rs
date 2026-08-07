//! Simulated penumbra runner: emits the same progress stream as a real run
//! without opening a device or touching hardware. Mirrors the mtk
//! `SimulatedMtkRunner` pattern.

use crate::penumbra::ops::{
    EventCb, PartitionEntry, PenumbraBootMode, PenumbraRunner, emit_done, emit_phase,
};
use crate::penumbra::Result;
use penumbra::core::storage::PartitionKind;
use std::path::Path;
use std::thread;
use std::time::Duration;

/// Simulated partition size used for read/write ops.
const SIM_PARTITION_SIZE: u64 = 128 * 1024 * 1024;

/// How long a simulated run sleeps per 1 MiB chunk (inverse of ~50 MiB/s).
const SIM_CHUNK_SLEEP: Duration = Duration::from_millis(1);

/// `--simulate` runner.
#[derive(Default)]
pub struct SimulatedPenumbra;

impl SimulatedPenumbra {
    fn emit_progress(on_event: EventCb<'_>, total: u64) {
        emit_phase(on_event, "simulate", "simulated device");
        let mut pos = 0u64;
        let chunk = 1024 * 1024;
        while pos < total {
            pos = (pos + chunk).min(total);
            on_event(&crate::penumbra::types::PenumbraEvent::Progress { bytes: pos, total });
            thread::sleep(SIM_CHUNK_SLEEP);
        }
    }

    fn fake_entries() -> Vec<PartitionEntry> {
        vec![
            PartitionEntry {
                name: "boot".into(),
                address: 0x1000,
                size: SIM_PARTITION_SIZE,
                section: "EMMC-USER".into(),
            },
            PartitionEntry {
                name: "userdata".into(),
                address: 0x2000,
                size: SIM_PARTITION_SIZE * 8,
                section: "EMMC-USER".into(),
            },
        ]
    }
}

impl PenumbraRunner for SimulatedPenumbra {
    fn read_partition(&self, partition: &str, _file: &Path, on_event: EventCb<'_>) -> Result<u64> {
        emit_phase(on_event, "read", &format!("reading {partition} (simulated)"));
        Self::emit_progress(on_event, SIM_PARTITION_SIZE);
        emit_done(on_event, true, format!("simulated read of {partition}"));
        Ok(SIM_PARTITION_SIZE)
    }

    fn write_partition(&self, partition: &str, _file: &Path, on_event: EventCb<'_>) -> Result<u64> {
        emit_phase(on_event, "write", &format!("writing {partition} (simulated)"));
        Self::emit_progress(on_event, SIM_PARTITION_SIZE);
        emit_done(on_event, true, format!("simulated write of {partition}"));
        Ok(SIM_PARTITION_SIZE)
    }

    fn erase_partition(&self, partition: &str, on_event: EventCb<'_>) -> Result<()> {
        emit_phase(on_event, "erase", &format!("erasing {partition} (simulated)"));
        emit_done(on_event, true, format!("simulated erase of {partition}"));
        Ok(())
    }

    fn download_flash(&self, partition: &str, _file: &Path, on_event: EventCb<'_>) -> Result<()> {
        emit_phase(on_event, "download", &format!("flashing {partition} (simulated)"));
        Self::emit_progress(on_event, SIM_PARTITION_SIZE);
        emit_done(on_event, true, format!("simulated flash of {partition}"));
        Ok(())
    }

    fn upload(&self, partition: &str, _file: &Path, on_event: EventCb<'_>) -> Result<()> {
        emit_phase(on_event, "upload", &format!("reading {partition} (simulated)"));
        Self::emit_progress(on_event, SIM_PARTITION_SIZE);
        emit_done(on_event, true, format!("simulated readback of {partition}"));
        Ok(())
    }

    fn format(&self, partition: &str, on_event: EventCb<'_>) -> Result<()> {
        emit_phase(on_event, "format", &format!("formatting {partition} (simulated)"));
        emit_done(on_event, true, format!("simulated format of {partition}"));
        Ok(())
    }

    fn read_offset(&self, address: u64, length: usize, _file: &Path, on_event: EventCb<'_>) -> Result<()> {
        emit_phase(on_event, "read-offset", &format!("reading 0x{address:X} (simulated)"));
        Self::emit_progress(on_event, length as u64);
        emit_done(on_event, true, format!("simulated read of {length} bytes"));
        Ok(())
    }

    fn write_offset(&self, address: u64, _section: PartitionKind, _file: &Path, on_event: EventCb<'_>) -> Result<()> {
        emit_phase(on_event, "write-offset", &format!("writing 0x{address:X} (simulated)"));
        emit_done(on_event, true, "simulated write".into());
        Ok(())
    }

    fn erase_offset(&self, address: u64, length: usize, on_event: EventCb<'_>) -> Result<()> {
        emit_phase(on_event, "erase-offset", &format!("erasing 0x{address:X} (simulated)"));
        emit_done(on_event, true, format!("simulated erase of {length} bytes"));
        Ok(())
    }

    fn read_all(&self, _dir: &Path, skip: &[String], on_event: EventCb<'_>) -> Result<()> {
        for entry in Self::fake_entries() {
            if skip.contains(&entry.name) {
                emit_phase(on_event, "read-all", &format!("skipping {} (simulated)", entry.name));
                continue;
            }
            emit_phase(on_event, "read-all", &format!("reading {} (simulated)", entry.name));
            Self::emit_progress(on_event, entry.size);
        }
        emit_done(on_event, true, "simulated read-all".into());
        Ok(())
    }

    fn write_all(&self, _dir: &Path, skip: &[String], _ignore_missing: bool, on_event: EventCb<'_>) -> Result<()> {
        for entry in Self::fake_entries() {
            if skip.contains(&entry.name) {
                emit_phase(on_event, "write-all", &format!("skipping {} (simulated)", entry.name));
                continue;
            }
            emit_phase(on_event, "write-all", &format!("flashing {} (simulated)", entry.name));
            Self::emit_progress(on_event, entry.size);
        }
        emit_done(on_event, true, "simulated write-all".into());
        Ok(())
    }

    fn pgpt(&self, on_event: EventCb<'_>) -> Result<Vec<PartitionEntry>> {
        emit_phase(on_event, "pgpt", "reading partition table (simulated)");
        emit_done(on_event, true, "simulated pgpt".into());
        Ok(Self::fake_entries())
    }

    fn seccfg(&self, unlock: bool, on_event: EventCb<'_>) -> Result<()> {
        let action = if unlock { "unlock" } else { "lock" };
        emit_phase(on_event, "seccfg", &format!("{action}ing bootloader (simulated)"));
        emit_done(on_event, true, format!("simulated seccfg {action}"));
        Ok(())
    }

    fn peek(&self, address: u32, length: usize, _file: &Path, on_event: EventCb<'_>) -> Result<()> {
        emit_phase(on_event, "peek", &format!("peeking 0x{address:X} (simulated)"));
        Self::emit_progress(on_event, length as u64);
        emit_done(on_event, true, format!("simulated peek of {length} bytes"));
        Ok(())
    }

    fn poke(&self, address: u32, _file: &Path, on_event: EventCb<'_>) -> Result<()> {
        emit_phase(on_event, "poke", &format!("poking 0x{address:X} (simulated)"));
        emit_done(on_event, true, "simulated poke".into());
        Ok(())
    }

    fn rpmb_read(&self, region: u8, _start_sector: u32, sectors: u32, _file: &Path, on_event: EventCb<'_>) -> Result<()> {
        emit_phase(on_event, "rpmb", &format!("reading RPMB region {region} (simulated)"));
        Self::emit_progress(on_event, u64::from(sectors) * 256);
        emit_done(on_event, true, "simulated RPMB read".into());
        Ok(())
    }

    fn rpmb_write(&self, region: u8, _start_sector: u32, sectors: u32, _file: &Path, on_event: EventCb<'_>) -> Result<()> {
        emit_phase(on_event, "rpmb", &format!("writing RPMB region {region} (simulated)"));
        Self::emit_progress(on_event, u64::from(sectors) * 256);
        emit_done(on_event, true, "simulated RPMB write".into());
        Ok(())
    }

    fn rpmb_auth(&self, region: u8, _key_hex: &str, on_event: EventCb<'_>) -> Result<()> {
        emit_phase(on_event, "rpmb", &format!("authenticating RPMB region {region} (simulated)"));
        emit_done(on_event, true, "simulated RPMB auth".into());
        Ok(())
    }

    fn reboot(&self, mode: PenumbraBootMode, on_event: EventCb<'_>) -> Result<()> {
        emit_phase(on_event, "reboot", &format!("rebooting into {mode:?} (simulated)"));
        emit_done(on_event, true, "simulated reboot".into());
        Ok(())
    }

    fn shutdown(&self, on_event: EventCb<'_>) -> Result<()> {
        emit_phase(on_event, "shutdown", "shutting down (simulated)");
        emit_done(on_event, true, "simulated shutdown".into());
        Ok(())
    }

    fn set_active_slot(&self, slot_a: bool, on_event: EventCb<'_>) -> Result<()> {
        emit_phase(on_event, "set-slot", &format!("setting slot {} (simulated)", if slot_a { "a" } else { "b" }));
        emit_done(on_event, true, "simulated set-slot".into());
        Ok(())
    }

    fn crash(&self, on_event: EventCb<'_>) -> Result<()> {
        emit_phase(on_event, "crash", "crashing device (simulated)");
        emit_done(on_event, true, "simulated crash".into());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::penumbra::types::PenumbraEvent;

    #[test]
    fn simulated_read_emits_monotonic_progress() {
        let mut events = Vec::new();
        let bytes = SimulatedPenumbra
            .read_partition("boot", Path::new("/tmp/boot.img"), &mut |e| events.push(e.clone()))
            .unwrap();
        assert_eq!(bytes, SIM_PARTITION_SIZE);
        let progress: Vec<u64> = events
            .iter()
            .filter_map(|e| match e {
                PenumbraEvent::Progress { bytes, .. } => Some(*bytes),
                _ => None,
            })
            .collect();
        assert!(progress.windows(2).all(|w| w[0] <= w[1]));
        assert!(progress.last().copied().unwrap_or(0) >= SIM_PARTITION_SIZE);
        assert!(matches!(events.last(), Some(PenumbraEvent::Done { ok: true, .. })));
    }

    #[test]
    fn simulated_erase_returns_ok() {
        let mut events = Vec::new();
        SimulatedPenumbra
            .erase_partition("userdata", &mut |e| events.push(e.clone()))
            .unwrap();
        assert!(matches!(events.last(), Some(PenumbraEvent::Done { ok: true, .. })));
    }

    #[test]
    fn simulated_pgpt_lists_partitions() {
        let mut events = Vec::new();
        let entries = SimulatedPenumbra.pgpt(&mut |e| events.push(e.clone())).unwrap();
        assert!(entries.iter().any(|p| p.name == "boot"));
        assert!(events.len() >= 2);
    }

    #[test]
    fn simulated_seccfg_lock_and_unlock() {
        for unlock in [true, false] {
            let mut events = Vec::new();
            SimulatedPenumbra.seccfg(unlock, &mut |e| events.push(e.clone())).unwrap();
            assert!(matches!(events.last(), Some(PenumbraEvent::Done { ok: true, .. })));
        }
    }

    #[test]
    fn simulated_read_all_skips() {
        let mut events = Vec::new();
        let skip = vec!["boot".to_string()];
        SimulatedPenumbra.read_all(Path::new("/tmp"), &skip, &mut |e| events.push(e.clone())).unwrap();
        let skipped = events
            .iter()
            .any(|e| matches!(e, PenumbraEvent::Phase { phase, message } if phase == "read-all" && message.contains("skipping boot")));
        assert!(skipped);
    }
}
