//! DA-mode (`MediaTek` Download Agent) integration via a frozen mtkclient
//! "bridge" sidecar.
//!
//! # Overview
//!
//! A public fork of mtkclient (`ardiandideyashidiq/mtkclient`) publishes a
//! PyInstaller-frozen bridge as a GitHub release. pawflash:
//!
//! - fetches the pinned `manifest.json` ([`manifest`]),
//! - downloads + verifies + atomically installs the platform archive ([`install`]),
//! - spawns the bridge and parses its JSON-lines protocol ([`bridge`]),
//! - drives `read`/`write`/`erase` operations ([`ops`]).
//!
//! The module is synchronous end-to-end; async consumers wrap calls in
//! `tokio::task::spawn_blocking`.

/// Error types for the mtk bridge integration.
pub mod error;
/// Release manifest fetch and platform resolution.
pub mod manifest;
/// Download, verify, and atomic install of the frozen bridge.
pub mod install;
/// Bridge process runner and JSON-lines protocol parser.
pub mod bridge;
/// High-level read/write/erase operations.
pub mod ops;
/// Serde types mirroring the bridge protocol.
pub mod types;

pub use error::{MtkError, Result};
pub use install::{current_version, ensure_installed, install_root};
pub use manifest::{current_platform, fetch_manifest, Manifest, PlatformAsset};
pub use ops::{
    read_partition, write_partition, erase_partition, BridgeRunner, RealBridge, SimulatedMtkRunner,
};
pub use types::{MtkCommand, MtkEvent, MtkOutcome, PartType};
