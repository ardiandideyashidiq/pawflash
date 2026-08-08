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

    // ---- parse file header ----
    let mut header_bytes = FileHeaderBytes::default();
    file.read_exact(&mut header_bytes).await?;
    let header = FileHeader::from_bytes(&header_bytes)
        .map_err(|_| FlashError::SparseParseFailed)?;

    // ---- parse all chunk headers, skipping data ----
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
        let mut sender = tokio::time::timeout(timeout, fb.download(sparse_size))
            .await
            .map_err(|_| FlashError::Timeout { partition: partition.into(), step: "download".into() })??;

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
                    file_pos += direct.len() as u64;
                    if let Some(rep) = reporter.as_mut() {
                        rep.inc(direct.len() as u64);
                    }
                    written += direct.len() as u64;
                    remaining = remaining.saturating_sub(direct.len());
                }
            }
            if let Some(rep) = reporter.as_mut() {
                rep.report(written, total_download);
            }
        }

        sender.finish().await?;
        last_resp = fb.flash(partition).await?;
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
    // Running file offset; chunk data is contiguous within a split, so only
    // seek when a chunk target diverges (split_raw never reorders chunks, but
    // keeping this explicit is harmless and cheap).
    let mut file_pos: u64 = 0;
    for (i, split) in splits.iter().enumerate() {
        if reporter.as_ref().is_some_and(|r| r.cancelled()) {
            return Err(FlashError::Cancelled);
        }
        debug!(%partition, part = i, "sending sparse-wrapped split");

        let sparse_size = u32::try_from(split.sparse_size())
            .map_err(|_| FlashError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "sparse split size exceeds u32 range",
            )))?;
        info!(%partition, part = i, sparse_size, max_download = limits.max_download, "downloading split via fb.download");
        let timeout = limits.transfer_timeout.unwrap_or(TRANSFER_TIMEOUT);
        let mut sender = tokio::time::timeout(timeout, fb.download(sparse_size))
            .await
            .map_err(|_| FlashError::Timeout { partition: partition.into(), step: "download".into() })??;
        info!(%partition, part = i, "fb.download returned successfully");

        // file header for this split
        sender.extend_from_slice(&split.header.to_bytes()).await?;
        written += FILE_HEADER_BYTES_LEN as u64;

        // chunk headers + data for each chunk in this split
        for chunk in &split.chunks {
            sender.extend_from_slice(&chunk.header.to_bytes()).await?;
            written += CHUNK_HEADER_BYTES_LEN as u64;

            if chunk.size > 0 {
                let target = u64::try_from(chunk.offset).unwrap_or(0);
                if target != file_pos {
                    file.seek(SeekFrom::Start(target)).await?;
                    file_pos = target;
                }

                let mut remaining = chunk.size;
                while remaining > 0 {
                    if reporter.as_ref().is_some_and(|r| r.cancelled()) {
                        return Err(FlashError::Cancelled);
                    }
                    let to_read = buf.get(1024 * 1024).len().min(remaining);
                    // Read directly into the USB buffer, skipping the
                    // intermediate transfer-buffer copy.
                    let direct = sender.get_mut_data(to_read).await?;
                    // Use plain read_exact_padded here (not the truncation-check
                    // variant) because split_raw may create chunks that extend
                    // past the end of the file for block alignment.  Zero-filling
                    // the tail is correct.
                    read_exact_padded(&mut file, direct).await?;
                    file_pos += direct.len() as u64;
                    written += direct.len() as u64;
                    remaining = remaining.saturating_sub(direct.len());
                }
            }
            if let Some(rep) = reporter.as_mut() {
                rep.report(written, total_sent);
            }
        }

        sender.finish().await?;
        last_resp = fb.flash(partition).await?;
    }

    if let Some(rep) = reporter.as_mut() {
        rep.set_position(total_sent);
        rep.report(total_sent, total_sent);
    }

    debug!(%partition, splits = splits.len(), response = last_resp, "sparse-wrapped flash complete");
    Ok(last_resp)
}


