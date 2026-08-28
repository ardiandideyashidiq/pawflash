//! Cross-platform abstraction for OS-varying behavior.
//!
//! All platform-specific logic (serial port names, data directories,
//! driver management, udev rules) is routed through the [`Platform`]
//! trait. A single `const CURRENT` instance is selected at compile time.

mod linux;
mod windows;

/// Platform-specific behavior contract.
pub trait Platform {
    /// Whether this serial port name is a candidate preloader port.
    fn is_candidate_serial_port(&self, name: &str) -> bool;

    /// Base data directory for pawflash (before module-specific subdir).
    fn base_data_dir(&self) -> std::path::PathBuf;

    /// Bridge executable name (`"bridge"` or `"bridge.exe"`).
    fn bridge_binary_name(&self) -> &'static str;

    /// Manifest platform key (`"linux-x86_64"` or `"windows-x86_64"`).
    ///
    /// # Errors
    ///
    /// Returns `Err` for unsupported platforms.
    fn manifest_platform_key(&self) -> Result<&'static str, String>;

    /// Ensure platform-specific USB driver prerequisites.
    /// No-op on Linux; checks/installs USBDK on Windows.
    ///
    /// # Errors
    ///
    /// Returns `Err` with a human-readable message on failure.
    fn ensure_driver(&self) -> Result<(), String>;

    /// Whether the USBDK driver is installed.
    fn usbdk_installed(&self) -> bool;

    /// Platform-appropriate permission error patterns.
    fn permission_error_patterns(&self) -> &'static [&'static str];

    /// Whether to accept `SerialPortType::Unknown` as a candidate.
    /// `true` on Windows (usbser driver), `false` on Linux.
    fn accept_unknown_port_type(&self) -> bool;

    /// Install udev rules for `MediaTek` devices. Returns `true` if already
    /// present or freshly installed.
    fn install_udev_rules(&self) -> bool;

    /// Add current user to dialout/plugdev groups. Returns `true` if updated.
    fn add_user_to_group(&self) -> bool;

    /// Print manual setup guidance to the user.
    fn print_manual_guidance(&self);

    /// Windows-specific hint text after fastboot handshake.
    /// Empty string on Linux.
    fn post_handshake_hint(&self) -> &'static str;
}

/// Compile-time selected platform implementation.
#[cfg(target_os = "linux")]
pub const CURRENT: &dyn Platform = &linux::Impl;

#[cfg(target_os = "windows")]
pub const CURRENT: &dyn Platform = &windows::Impl;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub const CURRENT: &dyn Platform = &linux::Impl;
