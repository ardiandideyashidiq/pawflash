use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use indicatif::ProgressBar;
use tokio::io::AsyncReadExt;
use tracing::{debug, info, warn};

use crate::flash::error::{FlashError, Result};
use crate::flash::progress::{FlashRunOptions, FlashTransferEvent, TransferReporter};
use crate::flash::results::{FlashOutcome, FlashResult};
use crate::flash::transport::FlashTransport;
use crate::scatter_parser::types::FlashPlan;
use super::FlashExecutor;
use super::EMPTY_VBMETA;

/// Transfer chunk size used when streaming files into the USB buffer.
const TRANSFER_CHUNK: u64 = 1024 * 1024;

impl<T: FlashTransport> FlashExecutor<T> {
    /// # Errors
    /// Returns an error if the download or flash command fails.
    ///
    /// # Panics
    /// Panics if `EMPTY_VBMETA` exceeds 4 GiB (impossible for a 512-byte image).
    pub async fn flash_empty_vbmeta(&mut self) -> Result<String> {
        // vbmeta is an A/B partition on virtually all devices; verify the
        // device actually has it slotted before flashing both halves.
        if let Ok(has_slot) = self.fb.get_var("has-slot:vbmeta").await {
            if has_slot != "yes" {
                return Err(FlashError::ActionFailed {
                    partition: "vbmeta".into(),
                    reason: format!("device has-slot:vbmeta is '{has_slot}', expected 'yes'"),
                });
            }
        }
        let data = EMPTY_VBMETA;
        debug!("flashing empty vbmeta to both slots");
        let mut last_resp = String::new();
        for slot in &["a", "b"] {
            let partition = format!("vbmeta_{slot}");
            info!(%partition, "flashing empty vbmeta");
            let mut sender = self.fb.download(
                u32::try_from(data.len())
                    .expect("EMPTY_VBMETA is 512 bytes, always fits in u32"),
            ).await?;
            sender.extend_from_slice(data).await?;
            sender.finish().await?;
            last_resp = self.fb.flash(&partition).await?;
        }
        Ok(last_resp)
    }

    /// Flash a raw image to a partition. Public entry point for `flash-raw`.
    /// Returns the device response message.
    ///
    /// # Errors
    ///
    /// Returns an error if the image file cannot be read, the device cannot
    /// accept the data, or the flash command fails.
    pub async fn flash_raw_image(
        &mut self,
        partition: &str,
        image_path: &Path,
    ) -> Result<String> {
        debug!(%partition, image_path = %image_path.display(), "flash_raw_image entry");
        let max_download = self.max_download().await?;

        self.flash_image_to_partition(partition, image_path, max_download, None).await
    }

    /// Shared helper: erase partition, then download+flash (single or chunked).
    /// Detects Android sparse images and routes to the sparse-aware handler.
    /// Returns the device response message.
    async fn flash_image_to_partition(
        &mut self,
        partition: &str,
        path: &Path,
        max_download: u32,
        mut reporter: Option<&mut TransferReporter<'_>>,
    ) -> Result<String> {
        // Shared transfer buffer reused across all sparse operations.
        let mut xbuf = crate::flash::sparse::XferBuf::new();

        // Route Android sparse images through the sparse-aware handler.
        if crate::flash::sparse::is_sparse_image(path).await.unwrap_or(false) {
            let file_len = tokio::fs::metadata(path).await?.len();
            return crate::flash::sparse::flash_sparse_image(
                &mut self.fb,
                partition,
                path,
                file_len,
                max_download,
                reporter,
                &mut xbuf,
            )
            .await;
        }

        let file_len = tokio::fs::metadata(path).await?.len();
        let size = u32::try_from(file_len).unwrap_or(u32::MAX);

        if let Some(rep) = reporter.as_mut() {
            rep.set_length(file_len);
            rep.set_prefix(partition);
            rep.reset();
            rep.set_position(0);
        }

        debug!(%partition, file_size = file_len, max_download, "flashing image to partition");

        if size > max_download {
            // Route through sparse wrapping to avoid each flash overwriting from
            // offset 0 (the fastbootd flash handler writes downloaded data at the
            // start of the partition; raw chunked flash would only leave the last
            // chunk intact).  Sparse wrapping encodes offset metadata so the device
            // writes each split to the correct position, matching AOSP behaviour.
            info!(%partition, file_len, %max_download, "image exceeds max download, wrapping in sparse format");
            crate::flash::sparse::flash_sparse_wrapped(
                &mut self.fb,
                partition,
                path,
                file_len,
                max_download,
                reporter,
                &mut xbuf,
            )
            .await
        } else {
            self.flash_raw_partition(partition, path, size, reporter, &mut xbuf).await
        }
    }

    /// # Errors
    /// Returns an error if the fastboot query fails.
    pub async fn execute_plan(
        &mut self,
        plan: &FlashPlan,
        mut opts: FlashRunOptions<'_>,
    ) -> FlashResult {
        let all_actions: Vec<_> = plan.actions.iter().filter(|a| a.action == "flash").collect();
        let total = all_actions.len();
        if opts.dry_run {
            info!(total, "DRY RUN — no data will be written");
        } else {
            info!(total, "starting flash execution");
        }
        let max_download = match self.max_download().await {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, "falling back to default max-download-size");
                256 * 1024 * 1024
            }
        };
        if !opts.dry_run {
            // Activate the slot the plan actually targets instead of blindly
            // forcing slot "a" — a b-only plan (or a non-A/B device) must not
            // end up with the wrong slot active.
            if let Some(slot) = plan.actions.iter().find_map(|a| a.slot.as_deref()) {
                match self.set_active_slot(slot).await {
                    Ok(response) => info!(slot, response, "active slot set"),
                    Err(e) => warn!(slot, error = %e, "set_active failed; continuing"),
                }
            }
        }

        let mut outcomes = Vec::with_capacity(total);
        let completed_bytes = AtomicU64::new(0);
        let current_bytes = AtomicU64::new(0);
        let mut cancelled = false;

        for action in &all_actions {
            if opts.cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                info!(partition = %action.partition, "cancellation requested before partition");
                cancelled = true;
                break;
            }

            let partition = &action.partition;
            info!(%partition, "Writing partition");

            // Per-partition byte reporter folding cumulative overall progress.
            current_bytes.store(0, Ordering::Relaxed);
            let mut on_bytes: Option<Box<dyn FnMut(u64, u64) + Send + '_>> = None;
            if let Some(cb) = opts.on_transfer.as_mut() {
                let callback = &mut **cb;
                on_bytes = Some(Box::new(|bytes: u64, total: u64| {
                    current_bytes.store(bytes, Ordering::Relaxed);
                    let completed = completed_bytes.load(Ordering::Relaxed);
                    callback(FlashTransferEvent {
                        partition: partition.clone(),
                        operation: "flash".into(),
                        bytes,
                        total,
                        overall_bytes: completed + bytes,
                        overall_total: completed + total,
                    });
                }));
            }

            let pb = opts
                .progress
                .map(|_| crate::output::spinner::partition_progress_bar(partition));
            let mut reporter = TransferReporter::new(
                pb.as_ref(),
                on_bytes
                    .as_mut()
                    .map(|c| c.as_mut() as &mut (dyn FnMut(u64, u64) + Send)),
            );

            let start = Instant::now();
            let result = self
                .flash_partition(action, opts.dry_run, max_download, Some(&mut reporter))
                .await;
            let duration = start.elapsed();

            let partition_bytes = current_bytes.load(Ordering::Relaxed);
            // Advance the cumulative total by the bytes actually transferred
            // (not the nominal partition total) so a partition that fails
            // part-way does not inflate the overall byte count.
            completed_bytes.fetch_add(partition_bytes, Ordering::Relaxed);

            outcomes.push(record_outcome(partition, duration, result, pb.as_ref()));
        }

        let succeeded = outcomes.iter().filter(|o| o.success).count();
        let failed = outcomes.iter().filter(|o| !o.success).count();
        if cancelled {
            info!(succeeded, failed, total, "flash plan execution cancelled");
        } else {
            info!(succeeded, failed, total, "flash plan execution complete");
        }
        FlashResult {
            // On cancellation only the outcomes already processed count, so
            // `succeeded + failed == total` always holds.
            total: if cancelled { outcomes.len() } else { total },
            succeeded,
            failed,
            outcomes,
            cancelled,
        }
    }

    async fn flash_partition(
        &mut self,
        action: &crate::scatter_parser::types::FlashAction,
        dry_run: bool,
        max_download: u32,
        reporter: Option<&mut TransferReporter<'_>>,
    ) -> Result<String> {
        let partition = &action.partition;
        let Some(image_path) = action.image_resolved_path() else {
            return Err(FlashError::ActionFailed {
                partition: partition.clone(),
                reason: "no resolved image path".into(),
            });
        };

        let path = Path::new(image_path);
        if !path.exists() {
            return Err(FlashError::ImageNotFound(path.to_path_buf()));
        }

        // Runtime oversized-image guard. The plan-level `fits_partition` check
        // only runs when `--check-images` is on, so enforce the hard limit
        // here for non-sparse images (sparse images carry their own extent and
        // the device validates them).
        let file_len = tokio::fs::metadata(path).await?.len();
        if !crate::flash::sparse::is_sparse_image(path).await.unwrap_or(false)
            && u64::try_from(action.size)
                .is_ok_and(|limit| limit > 0 && file_len > limit)
        {
            return Err(FlashError::ImageTooLarge {
                name: partition.clone(),
                image_size: file_len,
                partition_size: action.size,
            });
        }

        debug!(
            %partition,
            %image_path,
            max_download,
            "checking image"
        );

        if dry_run {
            info!(%partition, %image_path, size = file_len, "dry run: would flash this image");
            return Ok(String::new());
        }

        self.flash_image_to_partition(partition, path, max_download, reporter).await
    }

    /// Flash a partition that fits in a single download.
    /// Returns the device response message.
    pub(crate) async fn flash_raw_partition(
        &mut self,
        partition: &str,
        path: &Path,
        size: u32,
        mut reporter: Option<&mut TransferReporter<'_>>,
        _xbuf: &mut crate::flash::sparse::XferBuf,
    ) -> Result<String> {
        debug!(%partition, file_size = size, "flashing raw partition");
        let mut file = tokio::fs::File::open(path).await?;
        let mut sender = self.fb.download(size).await?;

        // Read the file directly into the USB transfer buffer, avoiding the
        // intermediate copy of `extend_from_slice`. `get_mut_data` reserves
        // bytes against the download budget; `read_exact` fills the reserved
        // slice, and `size` (from metadata) guarantees we never reserve more
        // than the file holds.
        let mut written = 0u64;
        while written < u64::from(size) {
            let remaining = u64::from(size) - written;
            let want = usize::try_from(remaining.min(TRANSFER_CHUNK)).unwrap_or(usize::MAX);
            let buf = sender.get_mut_data(want).await?;
            file.read_exact(buf).await?;
            let n = buf.len() as u64;
            written += n;
            if let Some(rep) = reporter.as_mut() {
                rep.inc(n);
                rep.report(written, u64::from(size));
            }
        }

        sender.finish().await?;
        let resp = self.fb.flash(partition).await?;
        if let Some(rep) = reporter.as_mut() {
            rep.set_position(u64::from(size));
            rep.report(u64::from(size), u64::from(size));
        }
        debug!(%partition, response = resp, "raw partition flash complete");
        Ok(resp)
    }
}

/// Convert a single partition flash result into a recorded outcome, finishing
/// (or abandoning) the CLI progress bar for that partition.
fn record_outcome(
    partition: &str,
    duration: Duration,
    result: crate::flash::error::Result<String>,
    pb: Option<&ProgressBar>,
) -> FlashOutcome {
    match result {
        Ok(response) => {
            info!(%partition, duration = ?duration, response, "flash successful");
            if let Some(pb) = pb {
                pb.finish();
            }
            FlashOutcome {
                partition: partition.to_string(),
                success: true,
                response: Some(response),
                duration,
                error: None,
            }
        }
        Err(e) => {
            warn!(%partition, duration = ?duration, error = %e, "flash failed, skipping");
            if let Some(pb) = pb {
                pb.abandon_with_message(format!("{partition} failed"));
            }
            FlashOutcome {
                partition: partition.to_string(),
                success: false,
                response: None,
                duration,
                error: Some(e),
            }
        }
    }
}
