//! `pawflash` — MTK device flashing toolkit.
//!
//! # Modules
//!
//! - [`force_fastboot`] — force a device into fastboot mode via preloader serial
//! - [`scatter_parser`] — parse `MediaTek` scatter manifests and build flash plans
//! - [`flash`] — execute flash plans via fastboot protocol
//! - [`mtk`] — DA-mode partition ops via the frozen mtkclient bridge
//! - [`udev`] — Linux udev rules for mtkclient DA devices
//! - [`cli`] — CLI handlers for each subcommand

/// Fastboot flash execution.
pub mod flash;
/// Preloader serial fastboot mode negotiation.
pub mod force_fastboot;
/// DA-mode partition read/write/erase via the mtkclient bridge.
pub mod mtk;

/// User-facing output formatting, status lines, and tables.
pub mod output;
/// MediaTek scatter manifest parser and flash-plan builder.
pub mod scatter_parser;
/// Linux udev rules for mtkclient DA USB devices.
pub mod udev;
