use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;

use crate::flash::error::Result;
use crate::flash::transport::DownloadSender;
use crate::flash::transport::FlashTransport;
use crate::scatter_parser::types::ScatterFile;

// ── Download sink ──────────────────────────────────────────────────

/// Accumulates downloaded data without simulated USB transfer delay.
///
/// Simulation runs at the storage's native read speed: the executor reads
/// image bytes from disk directly into this buffer, so wall-clock time
/// reflects real filesystem throughput (like a file copy) rather than a
/// fixed device profile.
#[derive(Default)]
pub struct SimulatedDownloadSink {
    data: Vec<u8>,
}

impl SimulatedDownloadSink {
    /// Append a data chunk. No simulated delay — disk I/O bounds the rate.
    ///
    /// # Errors
    /// Returns an error if the data chunk is malformed (never in practice).
    pub fn extend_from_slice(&mut self, data: &[u8]) -> Result<()> {
        self.data.extend_from_slice(data);
        Ok(())
    }

    /// Reserve up to `max` bytes for direct writes. The reserved bytes are
    /// committed to the buffer; the caller must fill them.
    ///
    /// # Errors
    /// Returns an error if reservation fails (never in practice).
    pub fn get_mut_data(&mut self, max: usize) -> Result<&mut [u8]> {
        let start = self.data.len();
        self.data.reserve(max);
        self.data.resize(start + max, 0);
        Ok(&mut self.data[start..])
    }

    /// Finalise the download.
    ///
    /// # Errors
    /// Returns an error if finalisation fails (never in practice).
    pub fn finish(self) -> Result<()> {
        Ok(())
    }

    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

// ── Simulated transport ────────────────────────────────────────────

/// Fastboot transport backed by simulation — no real device required.
///
/// Behaves like a real fastboot device but routes all I/O through a
/// simulated transport instead of actual USB hardware.
///
/// **Scatter flash uses real disk I/O**: image files are read from disk by
/// the executor and accumulated here, so wall-clock time reflects real
/// filesystem throughput — transfers run at the storage's native speed,
/// like a file copy. Only per-command latencies are simulated.
pub struct SimulatedTransport {
    device_vars: HashMap<String, String>,
    pub(crate) commands: Vec<String>,
    /// Accumulated downloaded bytes across all cycles (for metrics).
    total_downloaded: u64,
    /// Size of the last download payload (for metrics).
    last_download_size: u32,
}

impl SimulatedTransport {
    /// Create a transport with the given device variable responses.
    #[must_use]
    pub const fn new(device_vars: HashMap<String, String>) -> Self {
        Self {
            device_vars,
            commands: Vec::new(),
            total_downloaded: 0,
            last_download_size: 0,
        }
    }

    /// Build a transport whose `get_var` responses are seeded from a
    /// parsed scatter file: `partition-type:` and `partition-size:`
    /// for every partition, plus common device properties.
    #[must_use]
    pub fn from_scatter(scatter: &ScatterFile) -> Self {
        let mut vars = HashMap::new();

        vars.insert("max-download-size".into(), "0x10000000".into());
        vars.insert(
            "product".into(),
            format!(
                "SIM_{}",
                scatter.platform.as_deref().unwrap_or("MTK"),
            ),
        );
        vars.insert("serialno".into(), "SIM000001".into());
        vars.insert("version".into(), "0.5".into());
        vars.insert("current-slot".into(), "a".into());
        vars.insert("is-userspace".into(), "yes".into());

        for partitions in scatter.layouts.values() {
            for part in partitions {
                let name = &part.name;
                vars.insert(
                    format!("partition-type:{name}"),
                    part.image_type.clone().unwrap_or_else(|| "raw".into()),
                );
                vars.insert(
                    format!("partition-size:{name}"),
                    format!("{:#x}", part.size),
                );
            }
        }

        Self::new(vars)
    }

    /// Return the simulated device variables.
    #[must_use]
    pub const fn device_vars(&self) -> &HashMap<String, String> {
        &self.device_vars
    }
}

#[async_trait]
impl FlashTransport for SimulatedTransport {
    async fn get_var(&mut self, var: &str) -> Result<String> {
        self.commands.push(format!("SIM get_var:{var}"));
        tokio::time::sleep(Duration::from_millis(10)).await;
        self.device_vars.get(var).cloned().ok_or_else(|| {
            crate::flash::error::FlashError::ActionFailed {
                partition: var.to_string(),
                reason: format!("simulated: no value configured for '{var}'"),
            }
        })
    }

    async fn get_all_vars(&mut self) -> Result<HashMap<String, String>> {
        self.commands.push("SIM get_all_vars".into());
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(self.device_vars.clone())
    }

    async fn download(&mut self, size: u32) -> Result<DownloadSender<'_>> {
        self.commands.push(format!("SIM download:{size}"));
        self.last_download_size = size;
        Ok(DownloadSender::Simulated(SimulatedDownloadSink::default()))
    }

    async fn flash(&mut self, partition: &str) -> Result<String> {
        self.commands.push(format!("SIM flash:{partition}"));
        self.total_downloaded += u64::from(self.last_download_size);
        Ok(format!("OKAY flashing {partition}"))
    }

    async fn erase(&mut self, partition: &str) -> Result<String> {
        self.commands.push(format!("SIM erase:{partition}"));
        tokio::time::sleep(Duration::from_millis(200)).await;
        Ok(format!("OKAY erased {partition}"))
    }

    async fn reboot(&mut self) -> Result<()> {
        self.commands.push("SIM reboot".into());
        tokio::time::sleep(Duration::from_millis(500)).await;
        Ok(())
    }

    async fn reboot_to(&mut self, target: &str) -> Result<()> {
        self.commands.push(format!("SIM reboot_to:{target}"));
        tokio::time::sleep(Duration::from_secs(2)).await;
        Ok(())
    }

    async fn is_logical(&mut self, partition: &str) -> Result<bool> {
        self.commands.push(format!("SIM is_logical:{partition}"));
        Ok(partition == "metadata" || partition == "userdata" || partition == "cache")
    }

    async fn resize_logical_partition(&mut self, partition: &str, _size: u64) -> Result<()> {
        self.commands
            .push(format!("SIM resize_logical:{partition}:{_size}"));
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }

    async fn flashing(&mut self, cmd: &str) -> Result<String> {
        self.commands.push(format!("SIM flashing:{cmd}"));
        tokio::time::sleep(Duration::from_secs(1)).await;
        Ok(format!("OKAY flashing {cmd}"))
    }

    async fn set_active(&mut self, slot: &str) -> Result<String> {
        self.commands.push(format!("SIM set_active:{slot}"));
        tokio::time::sleep(Duration::from_millis(500)).await;
        Ok(format!("OKAY set_active {slot}"))
    }

    async fn snapshot_update(&mut self, cmd: &str) -> Result<String> {
        self.commands.push(format!("SIM snapshot_update:{cmd}"));
        tokio::time::sleep(Duration::from_millis(200)).await;
        Ok(format!("OKAY snapshot_update {cmd}"))
    }
}
