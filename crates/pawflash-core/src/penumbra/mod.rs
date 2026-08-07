//! Native DA-mode (`MediaTek` Download Agent) integration via the penumbra
//! library.
//!
//! # Overview
//!
//! Unlike the [`crate::mtk`] bridge (a frozen Python sidecar), penumbra is a
//! pure Rust library linked in-process. pawflash:
//!
//! - fetches the DA manifest from the penumbra fork ([`manifest`]),
//! - downloads + verifies + caches the platform DA file ([`da`]),
//! - persists the last-used DA selection ([`state`]),
//! - opens the device and validates DA/SoC compatibility ([`device`]),
//! - drives `read`/`write`/`erase`/`seccfg`/... operations ([`ops`]).
//!
//! The module is synchronous end-to-end; async consumers wrap calls in
//! `tokio::task::spawn_blocking`.

/// Error types for the penumbra integration.
pub mod error;
/// DA manifest fetch and device-name resolution.
pub mod manifest;
/// Platform data-dir resolution shared with the mtk module.
pub mod platform;
/// Serde event types consumed by the CLI and GUI.
pub mod types;

pub use error::{PenumbraError, Result};
pub use manifest::{fetch_da_manifest, list_dais, resolve_by_brand_chipset, resolve_by_device, DAEntry, DAManifest, DA_MANIFEST_URL};
pub use platform::{base_data_dir, penumbra_dir};
pub use types::PenumbraEvent;
