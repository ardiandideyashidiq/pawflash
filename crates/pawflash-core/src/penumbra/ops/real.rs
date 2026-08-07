//! Real penumbra runner: opens the device and drives operations through the
//! penumbra [`Device`] API.

use crate::penumbra::device::open_device;
use crate::penumbra::error::PenumbraError;
use crate::penumbra::ops::{
    EventCb, PartitionEntry, PenumbraBootMode, PenumbraRunner, emit_done, emit_phase,
    throttled_progress,
};
use crate::penumbra::Result;
use penumbra::core::bootctrl::{BootControl, BootPartition, OFFSET_SLOT_SUFFIX};
use penumbra::core::seccfg::LockFlag;
use penumbra::core::storage::{PartitionKind, RpmbRegion};
use penumbra::{Device, Storage};
use std::fs::{File, create_dir_all};
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;
use std::time::Duration;
use wincode::Serialize;

/// Default wait for an MTK device to appear.
const DEFAULT_WAIT: Duration = Duration::from_secs(30);

/// The real runner, holding the DA bytes needed to open the device.
pub struct RealPenumbra {
    da_bytes: Vec<u8>,
    wait: Duration,
}

impl RealPenumbra {
    /// Create a runner that opens devices with `da_bytes`.
    #[must_use]
    pub const fn new(da_bytes: Vec<u8>) -> Self {
        Self { da_bytes, wait: DEFAULT_WAIT }
    }

    /// Open the device, run `op`, and release the lock on drop.
    fn with_device<T>(&self, op: impl FnOnce(&mut Device) -> Result<T>) -> Result<T> {
        let mut dev = open_device(self.da_bytes.clone(), self.wait)?;
        let result = op(&mut dev.device);
        drop(dev);
        result
    }

    fn map_penumbra_err(e: &penumbra::error::Error) -> PenumbraError {
        PenumbraError::Penumbra(e.to_string())
    }

    /// Convert a `u64` file size to `usize`, erroring if it overflows.
    fn file_len_usize(file_size: u64) -> Result<usize> {
        usize::try_from(file_size)
            .map_err(|_| PenumbraError::Penumbra("file too large for this platform".into()))
    }
}

impl PenumbraRunner for RealPenumbra {
    fn read_partition(&self, partition: &str, file: &Path, on_event: EventCb<'_>) -> Result<u64> {
        self.with_device(|dev| {
            emit_phase(on_event, "read", &format!("reading {partition}"));
            let part = dev
                .dev_info
                .get_partition(partition)
                .ok_or_else(|| PenumbraError::Penumbra(format!("partition '{partition}' not found")))?;
            let total = part.size as u64;
            let f = File::create(file).map_err(|e| PenumbraError::Cache(e.to_string()))?;
            let mut writer = BufWriter::new(f);
            let mut cb = throttled_progress(on_event, total);
            dev.read_partition(partition, &mut writer, &mut cb)
                .map_err(|e| Self::map_penumbra_err(&e))?;
            drop(cb);
            writer.flush().map_err(|e| PenumbraError::Cache(e.to_string()))?;
            emit_done(on_event, true, format!("read {total} bytes"));
            Ok(total)
        })
    }

    fn write_partition(&self, partition: &str, file: &Path, on_event: EventCb<'_>) -> Result<u64> {
        self.with_device(|dev| {
            emit_phase(on_event, "write", &format!("writing {partition}"));
            let part = dev
                .dev_info
                .get_partition(partition)
                .ok_or_else(|| PenumbraError::Penumbra(format!("partition '{partition}' not found")))?;
            let total = part.size as u64;
            let f = File::open(file).map_err(|e| PenumbraError::Cache(e.to_string()))?;
            let mut reader = BufReader::new(f);
            let mut cb = throttled_progress(on_event, total);
            dev.write_partition(partition, &mut reader, &mut cb)
                .map_err(|e| Self::map_penumbra_err(&e))?;
            drop(cb);
            emit_done(on_event, true, format!("wrote {total} bytes"));
            Ok(total)
        })
    }

    fn erase_partition(&self, partition: &str, on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            emit_phase(on_event, "erase", &format!("erasing {partition}"));
            let mut cb = throttled_progress(on_event, 0);
            dev.erase_partition(partition, &mut cb).map_err(|e| Self::map_penumbra_err(&e))?;
            drop(cb);
            emit_done(on_event, true, format!("erased {partition}"));
            Ok(())
        })
    }

    fn download_flash(&self, partition: &str, file: &Path, on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            emit_phase(on_event, "download", &format!("flashing {partition} (SPFT)"));
            let part = dev
                .dev_info
                .get_partition(partition)
                .ok_or_else(|| PenumbraError::Penumbra(format!("partition '{partition}' not found")))?;
            let f = File::open(file).map_err(|e| PenumbraError::Cache(e.to_string()))?;
            let mut reader = BufReader::new(f);
            let file_size = std::fs::metadata(file)
                .map_err(|e| PenumbraError::Cache(e.to_string()))?
                .len();
            if file_size > part.size as u64 {
                return Err(PenumbraError::Penumbra(format!(
                    "file size ({file_size}) exceeds partition size ({})",
                    part.size
                )));
            }
            let mut cb = throttled_progress(on_event, file_size);
            dev.download(partition, Self::file_len_usize(file_size)?, &mut reader, &mut cb)
                .map_err(|e| Self::map_penumbra_err(&e))?;
            drop(cb);
            emit_done(on_event, true, format!("flashed {file_size} bytes"));
            Ok(())
        })
    }

    fn upload(&self, partition: &str, file: &Path, on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            emit_phase(on_event, "upload", &format!("reading {partition} (SPFT)"));
            let part = dev
                .dev_info
                .get_partition(partition)
                .ok_or_else(|| PenumbraError::Penumbra(format!("partition '{partition}' not found")))?;
            let total = part.size as u64;
            let f = File::create(file).map_err(|e| PenumbraError::Cache(e.to_string()))?;
            let mut writer = BufWriter::new(f);
            let mut cb = throttled_progress(on_event, total);
            dev.upload(partition, &mut writer, &mut cb).map_err(|e| Self::map_penumbra_err(&e))?;
            drop(cb);
            writer.flush().map_err(|e| PenumbraError::Cache(e.to_string()))?;
            emit_done(on_event, true, format!("read {total} bytes"));
            Ok(())
        })
    }

    fn format(&self, partition: &str, on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            emit_phase(on_event, "format", &format!("formatting {partition}"));
            let mut cb = throttled_progress(on_event, 0);
            dev.format(partition, &mut cb).map_err(|e| Self::map_penumbra_err(&e))?;
            drop(cb);
            emit_done(on_event, true, format!("formatted {partition}"));
            Ok(())
        })
    }

    fn read_offset(&self, address: u64, length: usize, file: &Path, on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            emit_phase(on_event, "read-offset", &format!("reading 0x{address:X}..0x{:X}", address + length as u64));
            let section = user_section(dev);
            let f = File::create(file).map_err(|e| PenumbraError::Cache(e.to_string()))?;
            let mut writer = BufWriter::new(f);
            let mut cb = throttled_progress(on_event, length as u64);
            dev.read_offset(address, length, section, &mut writer, &mut cb)
                .map_err(|e| Self::map_penumbra_err(&e))?;
            drop(cb);
            writer.flush().map_err(|e| PenumbraError::Cache(e.to_string()))?;
            emit_done(on_event, true, format!("read {length} bytes"));
            Ok(())
        })
    }

    fn write_offset(&self, address: u64, section: PartitionKind, file: &Path, on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            emit_phase(on_event, "write-offset", &format!("writing 0x{address:X}"));
            let f = File::open(file).map_err(|e| PenumbraError::Cache(e.to_string()))?;
            let mut reader = BufReader::new(f);
            let file_size = std::fs::metadata(file)
                .map_err(|e| PenumbraError::Cache(e.to_string()))?
                .len();
            let mut cb = throttled_progress(on_event, file_size);
            dev.write_offset(address, Self::file_len_usize(file_size)?, section, &mut reader, &mut cb)
                .map_err(|e| Self::map_penumbra_err(&e))?;
            drop(cb);
            emit_done(on_event, true, format!("wrote {file_size} bytes"));
            Ok(())
        })
    }

    fn erase_offset(&self, address: u64, length: usize, on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            emit_phase(on_event, "erase-offset", &format!("erasing 0x{address:X}"));
            let section = user_section(dev);
            let mut cb = throttled_progress(on_event, length as u64);
            dev.erase_offset(address, length, section, &mut cb).map_err(|e| Self::map_penumbra_err(&e))?;
            drop(cb);
            emit_done(on_event, true, format!("erased {length} bytes"));
            Ok(())
        })
    }

    fn read_all(&self, dir: &Path, skip: &[String], on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            create_dir_all(dir).map_err(|e| PenumbraError::Cache(e.to_string()))?;
            dev.enter_da_mode().map_err(|e| Self::map_penumbra_err(&e))?;
            let partitions = dev.get_partitions();
            for part in partitions {
                if skip.contains(&part.name) {
                    emit_phase(on_event, "read-all", &format!("skipping {}", part.name));
                    continue;
                }
                emit_phase(on_event, "read-all", &format!("reading {}", part.name));
                let out = dir.join(format!("{}.bin", part.name));
                let f = File::create(&out).map_err(|e| PenumbraError::Cache(e.to_string()))?;
                let mut writer = BufWriter::new(f);
                let mut cb = throttled_progress(on_event, part.size as u64);
                dev.upload(&part.name, &mut writer, &mut cb).map_err(|e| Self::map_penumbra_err(&e))?;
            drop(cb);
                writer.flush().map_err(|e| PenumbraError::Cache(e.to_string()))?;
            }
            emit_done(on_event, true, "read all partitions".into());
            Ok(())
        })
    }

    fn write_all(&self, dir: &Path, skip: &[String], ignore_missing: bool, on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            if !dir.is_dir() {
                return Err(PenumbraError::Cache(format!("{} is not a directory", dir.display())));
            }
            dev.enter_da_mode().map_err(|e| Self::map_penumbra_err(&e))?;
            let entries: Vec<(String, std::path::PathBuf)> = std::fs::read_dir(dir)
                .map_err(|e| PenumbraError::Cache(e.to_string()))?
                .filter_map(std::result::Result::ok)
                .filter_map(|e| {
                    let path = e.path();
                    if path.extension().and_then(|x| x.to_str()) != Some("bin") {
                        return None;
                    }
                    let name = path.file_stem()?.to_string_lossy().into_owned();
                    Some((name, path))
                })
                .collect();
            for (name, path) in entries {
                if skip.contains(&name) {
                    emit_phase(on_event, "write-all", &format!("skipping {name}"));
                    continue;
                }
                let Some(part) = dev.dev_info.get_partition(&name) else {
                    if ignore_missing {
                        emit_phase(on_event, "write-all", &format!("skipping {name} (absent)"));
                        continue;
                    }
                    return Err(PenumbraError::Penumbra(format!("partition '{name}' not found")));
                };
                let file_size = std::fs::metadata(&path)
                    .map_err(|e| PenumbraError::Cache(e.to_string()))?
                    .len();
                if file_size > part.size as u64 {
                    return Err(PenumbraError::Penumbra(format!(
                        "file size ({file_size}) exceeds partition size ({})",
                        part.size
                    )));
                }
                emit_phase(on_event, "write-all", &format!("flashing {name}"));
                let f = File::open(&path).map_err(|e| PenumbraError::Cache(e.to_string()))?;
                let mut reader = BufReader::new(f);
                let mut cb = throttled_progress(on_event, file_size);
                dev.download(&part.name, Self::file_len_usize(file_size)?, &mut reader, &mut cb)
                    .map_err(|e| Self::map_penumbra_err(&e))?;
            drop(cb);
            }
            emit_done(on_event, true, "wrote all partitions".into());
            Ok(())
        })
    }

    fn pgpt(&self, on_event: EventCb<'_>) -> Result<Vec<PartitionEntry>> {
        self.with_device(|dev| {
            emit_phase(on_event, "pgpt", "reading partition table");
            dev.enter_da_mode().map_err(|e| Self::map_penumbra_err(&e))?;
            let entries: Vec<PartitionEntry> = dev
                .get_partitions()
                .into_iter()
                .map(|p| PartitionEntry {
                    name: p.name,
                    address: p.address,
                    size: p.size as u64,
                    section: p.kind.as_str().to_string(),
                })
                .collect();
            emit_done(on_event, true, format!("{} partitions", entries.len()));
            Ok(entries)
        })
    }

    fn seccfg(&self, unlock: bool, on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            let action = if unlock { "unlock" } else { "lock" };
            emit_phase(on_event, "seccfg", &format!("{action}ing bootloader"));
            let flag = if unlock { LockFlag::Unlock } else { LockFlag::Lock };
            match dev.set_seccfg_lock_state(flag) {
                Some(_) => {
                    emit_done(on_event, true, format!("seccfg {action}ed"));
                    Ok(())
                }
                None => Err(PenumbraError::Penumbra(format!(
                    "failed to {action} seccfg (already in state or exploit unavailable)"
                ))),
            }
        })
    }

    fn peek(&self, address: u32, length: usize, file: &Path, on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            emit_phase(on_event, "peek", &format!("peeking 0x{address:X}"));
            let f = File::create(file).map_err(|e| PenumbraError::Cache(e.to_string()))?;
            let mut writer = BufWriter::new(f);
            let mut cb = throttled_progress(on_event, length as u64);
            dev.peek(address, length, &mut writer, &mut cb).map_err(|e| Self::map_penumbra_err(&e))?;
            drop(cb);
            writer.flush().map_err(|e| PenumbraError::Cache(e.to_string()))?;
            emit_done(on_event, true, format!("peeked {length} bytes"));
            Ok(())
        })
    }

    fn poke(&self, address: u32, file: &Path, on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            emit_phase(on_event, "poke", &format!("poking 0x{address:X}"));
            let f = File::open(file).map_err(|e| PenumbraError::Cache(e.to_string()))?;
            let mut reader = BufReader::new(f);
            let file_size = std::fs::metadata(file)
                .map_err(|e| PenumbraError::Cache(e.to_string()))?
                .len();
            let mut cb = throttled_progress(on_event, file_size);
            dev.poke(address, Self::file_len_usize(file_size)?, &mut reader, &mut cb)
                .map_err(|e| Self::map_penumbra_err(&e))?;
            drop(cb);
            emit_done(on_event, true, format!("poked {file_size} bytes"));
            Ok(())
        })
    }

    fn rpmb_read(&self, region: u8, start_sector: u32, sectors: u32, file: &Path, on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            emit_phase(on_event, "rpmb", &format!("reading RPMB region {region}"));
            let f = File::create(file).map_err(|e| PenumbraError::Cache(e.to_string()))?;
            let mut writer = BufWriter::new(f);
            let total = u64::from(sectors) * 256;
            let mut cb = throttled_progress(on_event, total);
            let rpmb_region = RpmbRegion::try_from(region).map_err(|_| {
                PenumbraError::Penumbra(format!("invalid RPMB region {region}"))
            })?;
            dev.read_rpmb(rpmb_region, start_sector, sectors, &mut writer, &mut cb)
                .map_err(|e| Self::map_penumbra_err(&e))?;
            drop(cb);
            writer.flush().map_err(|e| PenumbraError::Cache(e.to_string()))?;
            emit_done(on_event, true, format!("read {sectors} RPMB sectors"));
            Ok(())
        })
    }

    fn rpmb_write(&self, region: u8, start_sector: u32, sectors: u32, file: &Path, on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            emit_phase(on_event, "rpmb", &format!("writing RPMB region {region}"));
            let f = File::open(file).map_err(|e| PenumbraError::Cache(e.to_string()))?;
            let mut reader = BufReader::new(f);
            let total = u64::from(sectors) * 256;
            let mut cb = throttled_progress(on_event, total);
            let rpmb_region = RpmbRegion::try_from(region).map_err(|_| {
                PenumbraError::Penumbra(format!("invalid RPMB region {region}"))
            })?;
            dev.write_rpmb(rpmb_region, start_sector, sectors, &mut reader, &mut cb)
                .map_err(|e| Self::map_penumbra_err(&e))?;
            drop(cb);
            emit_done(on_event, true, format!("wrote {sectors} RPMB sectors"));
            Ok(())
        })
    }

    fn rpmb_auth(&self, region: u8, key_hex: &str, on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            emit_phase(on_event, "rpmb", "authenticating RPMB");
            let key = hex::decode(key_hex).map_err(|e| PenumbraError::Penumbra(e.to_string()))?;
            let rpmb_region = RpmbRegion::try_from(region).map_err(|_| {
                PenumbraError::Penumbra(format!("invalid RPMB region {region}"))
            })?;
            dev.auth_rpmb(rpmb_region, &key).map_err(|e| Self::map_penumbra_err(&e))?;
            emit_done(on_event, true, "RPMB authenticated".into());
            Ok(())
        })
    }

    fn reboot(&self, mode: PenumbraBootMode, on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            emit_phase(on_event, "reboot", &format!("rebooting into {mode:?}"));
            dev.reboot(mode.into()).map_err(|e| Self::map_penumbra_err(&e))?;
            emit_done(on_event, true, "reboot command sent".into());
            Ok(())
        })
    }

    fn shutdown(&self, on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            emit_phase(on_event, "shutdown", "shutting down device");
            dev.shutdown().map_err(|e| Self::map_penumbra_err(&e))?;
            emit_done(on_event, true, "shutdown command sent".into());
            Ok(())
        })
    }

    fn set_active_slot(&self, slot_a: bool, on_event: EventCb<'_>) -> Result<()> {
        self.with_device(|dev| {
            emit_phase(on_event, "set-slot", &format!("setting active slot {}", if slot_a { "a" } else { "b" }));
            let target = if slot_a { BootPartition::A } else { BootPartition::B };
            let mut bootctrl = dev.get_bootctrl().map_err(|e| Self::map_penumbra_err(&e))?;
            if bootctrl.get_active_slot() == target {
                emit_done(on_event, true, "active slot already set".into());
                return Ok(());
            }
            bootctrl.set_active_slot(target);
            let mut new_data = [0u8; OFFSET_SLOT_SUFFIX + size_of::<BootControl>()];
            BootControl::serialize_into(&mut new_data[OFFSET_SLOT_SUFFIX..], &bootctrl)
                .map_err(|e| PenumbraError::Penumbra(e.to_string()))?;
            let part = bootctrl.bctrl_part.clone();
            dev.download(&part, new_data.len(), &new_data[..], |_, _| {})
                .map_err(|e| Self::map_penumbra_err(&e))?;
            emit_done(on_event, true, format!("active slot set to {}", if slot_a { "a" } else { "b" }));
            Ok(())
        })
    }

    fn crash(&self, on_event: EventCb<'_>) -> Result<()> {
        emit_phase(on_event, "crash", "crashing device to bootrom");
        Err(PenumbraError::Penumbra(
            "crash is preloader-only and not implemented via the DA path".into(),
        ))
    }
}

/// The user-section `PartitionKind` for the connected storage, used by offset
/// ops that default to the user section (matching antumbra).
fn user_section(dev: &mut Device) -> PartitionKind {
    dev.dev_info.storage().map_or(PartitionKind::Unknown, |s| s.get_user_part())
}
