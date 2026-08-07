//! Linux udev rules for mtkclient DA (Download Agent) USB devices.
//!
//! The DA bridge talks to the device over raw USB (not a serial port), so a
//! non-root user needs udev rules granting access. This module installs them
//! idempotently (the rules content doubles as the "up to date" marker, so
//! `sudo` only runs when the file differs) and provides a USB-device
//! visibility check used by `pawflash mtkclient doctor`.

mod rules;

pub use rules::{device_visible, ensure_udev_rules, rules_content, DEVICE_VENDOR_IDS};
