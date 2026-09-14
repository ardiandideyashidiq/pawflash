use std::io::SeekFrom;
use std::path::Path;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncSeekExt};
use android_sparse_image::{
    split::{split_image, split_raw}, ChunkHeader, FileHeader, FileHeaderBytes,
    CHUNK_HEADER_BYTES_LEN, FILE_HEADER_BYTES_LEN,
};
use tracing::{debug, info};

use crate::flash::error::{FlashError, Result};
use crate::flash::progress::TransferReporter;
use crate::flash::transport::FlashTransport;

use super::{read_exact_padded, read_exact_padded_or_truncate, XferBuf};

/// Generous per-transfer-step timeout, mirroring the executor's constant.
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(300);

/// Flash a sparse image to a partition.
///
/// Parse the sparse file header + chunk headers, split into parts that each
/// fit within `max_download`, then send each part as a separate
async fn parse_sparse_header_and_chunks(
    file: &mut tokio::fs::File,
) -> Result<(FileHeader, Vec<ChunkHeader>)> {
    let mut header_bytes = FileHeaderBytes::default();
    file.read_exact(&mut header_bytes).await?;
    let header = FileHeader::from_bytes(&header_bytes)
        .map_err(|_| FlashError::SparseParseFailed)?;

    let mut chunks = Vec::with_capacity(header.chunks as usize);
    for _ in 0..header.chunks {
        let mut chunk_bytes = [0u8; CHUNK_HEADER_BYTES_LEN];
        file.read_exact(&mut chunk_bytes).await?;
        let chunk = ChunkHeader::from_bytes(&chunk_bytes)
            .map_err(|_| FlashError::SparseParseFailed)?;
        let data_size = chunk.data_size();
        if data_size > 0 {
            let seek_offset = i64::try_from(data_size)
                .map_err(|_| FlashError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "sparse chunk data size exceeds i64 range",
                )))?;
            file.seek(SeekFrom::Current(seek_offset)).await?;
        }
        chunks.push(chunk);
    }
    Ok((header, chunks))
}

/// Flash an Android sparse image to a partition, splitting into chunks that
/// fit within `max_download`.  Each split is sent as an independent
/// download+flash transaction.  The bootloader reassembles the pieces.
/// Returns the device response message from the final split flash.
pub(crate) async fn flash_sparse_image(
    fb: &mut impl FlashTransport,
    partition: &str,
    path: &Path,
    file_len: u64,
    limits: &crate::flash::sparse::TransferLimits,
    mut reporter: Option<&mut TransferReporter<'_>>,
    buf: &mut XferBuf,
) -> Result<String> {
    debug!(%partition, file_len, max_download = limits.max_download, "flashing sparse image");

    let mut file = tokio::fs::File::open(path).await?;
    let (header, chunks) = parse_sparse_header_and_chunks(&mut file).await?;

    info!(%partition, chunk_count = chunks.len(), "parsed sparse image header");

    // ---- split into max_download-sized pieces ----
    let splits = split_image(&header, &chunks, limits.max_download)
        .map_err(|_| FlashError::SparseSplitFailed)?;

    info!(%partition, split_count = splits.len(), "sparse image split for download");

    let total_download: u64 = splits.iter()
        .map(|s| u64::try_from(s.sparse_size()).unwrap_or(0))
        .sum();

    if let Some(rep) = reporter.as_mut() {
        rep.set_length(total_download);
        rep.set_prefix(partition);
        rep.reset();
        rep.set_position(0);
        rep.report(0, total_download);
    }

    // ---- flash each split (no erase — the flash command handles it) ----
    let mut last_resp = String::new();
    let mut written: u64 = 0;
    // Running file offset; sparse chunk data is contiguous within a split, so
    // we only need to seek when a DontCare chunk causes a jump.
    let mut file_pos: u64 = 0;
    for (i, split) in splits.iter().enumerate() {
        let t_split = std::time::Instant::now();
        if reporter.as_ref().is_some_and(|r| r.cancelled()) {
            return Err(FlashError::Cancelled);
        }
        debug!(%partition, part = i, "sending sparse split");

        let sparse_size = u32::try_from(split.sparse_size())
            .map_err(|_| FlashError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "sparse split size exceeds u32 range",
            )))?;
        let timeout = limits.transfer_timeout.unwrap_or(TRANSFER_TIMEOUT);
        let t_download = std::time::Instant::now();
        let mut sender = tokio::time::timeout(timeout, fb.download(sparse_size))
            .await
            .map_err(|_| FlashError::Timeout { partition: partition.into(), step: "download".into() })??;
        let download_init_duration = t_download.elapsed();

        // file header for this split
        sender.extend_from_slice(&split.header.to_bytes()).await?;
        if let Some(rep) = reporter.as_mut() {
            rep.inc(FILE_HEADER_BYTES_LEN as u64);
        }
        written += FILE_HEADER_BYTES_LEN as u64;

        // chunk headers + data for each chunk in this split
        for chunk in &split.chunks {
            sender.extend_from_slice(&chunk.header.to_bytes()).await?;
            if let Some(rep) = reporter.as_mut() {
                rep.inc(CHUNK_HEADER_BYTES_LEN as u64);
            }
            written += CHUNK_HEADER_BYTES_LEN as u64;

            if chunk.size > 0 {
                let target = u64::try_from(chunk.offset).unwrap_or(0);
                if target != file_pos {
                    file.seek(SeekFrom::Start(target)).await?;
                    file_pos = target;
                }

                let mut remaining = chunk.size;
                while remaining > 0 {
                    let to_read = buf.get(1024 * 1024).len().min(remaining);
                    // Read directly into the USB buffer, skipping the
                    // intermediate transfer-buffer copy.
                    let direct = sender.get_mut_data(to_read).await?;
                    read_exact_padded_or_truncate(&mut file, direct, chunk.size).await?;
                    let n = direct.len() as u64;
                    file_pos += n;
                    written += n;
                    if let Some(rep) = reporter.as_mut() {
                        rep.inc(n);
                        rep.report(written, total_download);
                    }
                    remaining = remaining.saturating_sub(direct.len());
                }
            }
            if let Some(rep) = reporter.as_mut() {
                rep.report(written, total_download);
            }
        }

        let t_finish = std::time::Instant::now();
        sender.finish().await?;
        let finish_duration = t_finish.elapsed();
        let t_flash_cmd = std::time::Instant::now();
        last_resp = fb.flash(partition).await?;
        let flash_cmd_duration = t_flash_cmd.elapsed();
        let split_duration = t_split.elapsed();
        info!(
            %partition,
            part = i + 1,
            total_parts = splits.len(),
            bytes = sparse_size,
            ?download_init_duration,
            ?finish_duration,
            ?flash_cmd_duration,
            ?split_duration,
            "sparse split completed"
        );
    }

    if let Some(rep) = reporter.as_mut() {
        rep.set_position(total_download);
        rep.report(total_download, total_download);
    }

    debug!(%partition, total_download, response = last_resp, "sparse flash complete");
    Ok(last_resp)
}

/// Flash a raw image by wrapping it in Android sparse format splits.
///
/// Uses `split_raw()` to convert the raw file into sparse-format splits
/// that each fit within `max_download`.  The bootloader expands them
/// on-device, avoiding transmission of large zero-filled regions.
/// Returns the device response message from the final split flash.
pub(crate) async fn flash_sparse_wrapped(
    fb: &mut impl FlashTransport,
    partition: &str,
    path: &Path,
    file_len: u64,
    limits: &crate::flash::sparse::TransferLimits,
    mut reporter: Option<&mut TransferReporter<'_>>,
    buf: &mut XferBuf,
) -> Result<String> {
    debug!(%partition, file_len, max_download = limits.max_download, "wrapping raw image in sparse format");

    let raw_size = usize::try_from(file_len)
        .map_err(|_| FlashError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file too large for split_raw",
        )))?;
    let splits = split_raw(raw_size, limits.max_download)
        .map_err(|_| FlashError::SparseSplitFailed)?;

    info!(%partition, split_count = splits.len(), "raw image split into sparse chunks");

    // The reporter measures the bytes actually sent over USB, which includes
    // the sparse file/chunk headers — report against that so the bar peaks
    // at exactly 100% instead of overshooting past the raw file length.
    let total_sent: u64 = splits
        .iter()
        .map(|s| u64::try_from(s.sparse_size()).unwrap_or(0))
        .sum();

    let mut file = tokio::fs::File::open(path).await?;

    if let Some(rep) = reporter.as_mut() {
        rep.set_length(total_sent);
        rep.set_prefix(partition);
        rep.reset();
        rep.set_position(0);
        rep.report(0, total_sent);
    }

    // ---- flash each split (no erase — the flash command handles it) ----
    let mut last_resp = String::new();
    let mut written: u64 = 0;
    let mut file_pos: u64 = 0;
    let total_splits = splits.len();
    let mut ctx = WrappedSplitContext {
        fb,
        partition,
        file: &mut file,
        file_pos: &mut file_pos,
        limits,
        reporter,
        buf,
        written: &mut written,
        total_sent,
        total_splits,
    };
    for (i, split) in splits.iter().enumerate() {
        last_resp = ctx.flash_split(split, i).await?;
    }

    if let Some(rep) = ctx.reporter.as_mut() {
        rep.set_position(total_sent);
        rep.report(total_sent, total_sent);
    }

    debug!(%partition, splits = total_splits, response = last_resp, "sparse-wrapped flash complete");
    Ok(last_resp)
}

struct WrappedSplitContext<'a, 'r, T: FlashTransport> {
    fb: &'a mut T,
    partition: &'a str,
    file: &'a mut tokio::fs::File,
    file_pos: &'a mut u64,
    limits: &'a crate::flash::sparse::TransferLimits,
    reporter: Option<&'a mut TransferReporter<'r>>,
    buf: &'a mut XferBuf,
    written: &'a mut u64,
    total_sent: u64,
    total_splits: usize,
}

impl<T: FlashTransport> WrappedSplitContext<'_, '_, T> {
    async fn flash_split(
        &mut self,
        split: &android_sparse_image::split::Split,
        index: usize,
    ) -> Result<String> {
        let t_split = std::time::Instant::now();
        if self.reporter.as_ref().is_some_and(|r| r.cancelled()) {
            return Err(FlashError::Cancelled);
        }
        debug!(partition = %self.partition, part = index, "sending sparse-wrapped split");

        let sparse_size = u32::try_from(split.sparse_size())
            .map_err(|_| FlashError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "sparse split size exceeds u32 range",
            )))?;
        let timeout = self.limits.transfer_timeout.unwrap_or(TRANSFER_TIMEOUT);
        let t_download = std::time::Instant::now();
        let mut sender = tokio::time::timeout(timeout, self.fb.download(sparse_size))
            .await
            .map_err(|_| FlashError::Timeout { partition: self.partition.into(), step: "download".into() })??;
        let download_init_duration = t_download.elapsed();

        // file header for this split
        sender.extend_from_slice(&split.header.to_bytes()).await?;
        if let Some(rep) = self.reporter.as_mut() {
            rep.inc(FILE_HEADER_BYTES_LEN as u64);
        }
        *self.written += FILE_HEADER_BYTES_LEN as u64;

        // chunk headers + data for each chunk in this split
        for chunk in &split.chunks {
            sender.extend_from_slice(&chunk.header.to_bytes()).await?;
            if let Some(rep) = self.reporter.as_mut() {
                rep.inc(CHUNK_HEADER_BYTES_LEN as u64);
            }
            *self.written += CHUNK_HEADER_BYTES_LEN as u64;

            if chunk.size > 0 {
                let target = u64::try_from(chunk.offset).unwrap_or(0);
                if target != *self.file_pos {
                    self.file.seek(SeekFrom::Start(target)).await?;
                    *self.file_pos = target;
                }

                let mut remaining = chunk.size;
                while remaining > 0 {
                    if self.reporter.as_ref().is_some_and(|r| r.cancelled()) {
                        return Err(FlashError::Cancelled);
                    }
                    let to_read = self.buf.get(1024 * 1024).len().min(remaining);
                    let direct = sender.get_mut_data(to_read).await?;
                    read_exact_padded(self.file, direct).await?;
                    let n = direct.len() as u64;
                    *self.file_pos += n;
                    *self.written += n;
                    if let Some(rep) = self.reporter.as_mut() {
                        rep.inc(n);
                        rep.report(*self.written, self.total_sent);
                    }
                    remaining = remaining.saturating_sub(direct.len());
                }
            }
            if let Some(rep) = self.reporter.as_mut() {
                rep.report(*self.written, self.total_sent);
            }
        }

        let t_finish = std::time::Instant::now();
        sender.finish().await?;
        let finish_duration = t_finish.elapsed();
        let t_flash_cmd = std::time::Instant::now();
        let resp = self.fb.flash(self.partition).await?;
        let flash_cmd_duration = t_flash_cmd.elapsed();
        let split_duration = t_split.elapsed();
        info!(
            partition = %self.partition,
            part = index + 1,
            total_parts = self.total_splits,
            bytes = sparse_size,
            ?download_init_duration,
            ?finish_duration,
            ?flash_cmd_duration,
            ?split_duration,
            "sparse-wrapped split completed"
        );
        Ok(resp)
    }
}


