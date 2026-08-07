//! Progress reporting and cancellation plumbing for flash transfers.
//!
//! The low-level transfer code used to drive only an `indicatif` CLI bar.
//! It now drives a [`TransferReporter`] that can additionally forward
//! byte-level updates through a callback (used by the Tauri GUI to stream
//! `Flashing` events with bytes/total and cumulative overall progress).

use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use indicatif::{MultiProgress, ProgressBar};

/// Throttle window for byte-progress callbacks (256 KiB).
const REPORT_THRESHOLD: u64 = 256 * 1024;

/// Minimum interval between byte-progress callbacks, so high-throughput
/// transfers do not flood the GUI with thousands of IPC messages per second.
const REPORT_INTERVAL: Duration = Duration::from_millis(100);

/// A single byte-level transfer update for one partition flash.
#[derive(Debug, Clone)]
pub struct FlashTransferEvent {
    /// Full partition name being flashed.
    pub partition: String,
    /// Operation kind (always `"flash"` for plan execution).
    pub operation: String,
    /// Bytes transferred so far for this partition.
    pub bytes: u64,
    /// Total bytes for this partition.
    pub total: u64,
    /// Cumulative bytes across all partitions processed so far.
    pub overall_bytes: u64,
    /// Cumulative total across all partitions processed so far.
    pub overall_total: u64,
}

/// Options controlling flash-plan execution.
#[derive(Default)]
pub struct FlashRunOptions<'a> {
    /// Run without writing anything to the device.
    pub dry_run: bool,
    /// Optional shared CLI progress bars (one bar per partition).
    pub progress: Option<&'a MultiProgress>,
    /// When set, execution stops cooperatively before the next partition.
    pub cancel: Option<&'a AtomicBool>,
    /// Optional byte-level transfer callback (e.g. Tauri event streaming).
    pub on_transfer: Option<&'a mut (dyn FnMut(FlashTransferEvent) + Send)>,
    /// Per-transfer-step timeout. `None` uses the default (300s); tests set a
    /// short value so the hang path can be exercised without waiting.
    pub transfer_timeout: Option<Duration>,
}

/// Drives both the CLI progress bar and an optional byte callback from the
/// same low-level transfer code, so the raw/sparse paths do not care about
/// the reporting destination.
pub(crate) struct TransferReporter<'a> {
    cli: Option<&'a ProgressBar>,
    on_bytes: Option<&'a mut (dyn FnMut(u64, u64) + Send)>,
    cancel: Option<&'a AtomicBool>,
    last_reported: u64,
    last_emit_at: Instant,
}

impl<'a> TransferReporter<'a> {
    pub(crate) fn new(
        cli: Option<&'a ProgressBar>,
        on_bytes: Option<&'a mut (dyn FnMut(u64, u64) + Send)>,
    ) -> Self {
        Self {
            cli,
            on_bytes,
            cancel: None,
            last_reported: 0,
            last_emit_at: Instant::now(),
        }
    }

    /// Attach the cancellation flag so the transfer loops can abort
    /// mid-partition when the user cancels.
    pub(crate) const fn with_cancel(mut self, cancel: Option<&'a AtomicBool>) -> Self {
        self.cancel = cancel;
        self
    }

    /// Whether the user has requested cancellation.
    pub(crate) fn cancelled(&self) -> bool {
        self.cancel.is_some_and(|f| f.load(std::sync::atomic::Ordering::Relaxed))
    }

    pub(crate) fn set_length(&mut self, len: u64) {
        if let Some(pb) = self.cli {
            pb.set_length(len);
        }
    }

    pub(crate) fn set_prefix(&mut self, prefix: &str) {
        if let Some(pb) = self.cli {
            pb.set_prefix(prefix.to_string());
        }
    }

    pub(crate) fn reset(&mut self) {
        self.last_reported = 0;
        self.last_emit_at = Instant::now();
        if let Some(pb) = self.cli {
            pb.reset();
        }
    }

    pub(crate) fn set_position(&mut self, pos: u64) {
        if let Some(pb) = self.cli {
            pb.set_position(pos);
        }
    }

    pub(crate) fn inc(&mut self, delta: u64) {
        if let Some(pb) = self.cli {
            pb.inc(delta);
        }
    }

    /// Forward `bytes`/`total` to the callback, throttled to at most one
    /// callback per `REPORT_INTERVAL` (and per `REPORT_THRESHOLD` bytes),
    /// except for the first (0) and last (>= total) reports so the UI gets a
    /// live but not overwhelming stream.
    pub(crate) fn report(&mut self, bytes: u64, total: u64) {
        let reached_end = bytes >= total;
        let advanced = bytes.saturating_sub(self.last_reported) >= REPORT_THRESHOLD;
        let fresh = self.last_emit_at.elapsed() >= REPORT_INTERVAL;
        if !(bytes == 0 || reached_end || (advanced && fresh)) {
            return;
        }
        self.last_reported = bytes;
        self.last_emit_at = Instant::now();
        if let Some(cb) = self.on_bytes.as_mut() {
            cb(bytes, total);
        }
    }
}
