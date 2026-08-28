//! Udev rule management and group-adding for MediaTek preloader serial ports.
//!
//! Platform-specific logic is delegated to [`crate::platform::CURRENT`].

/// Print manual setup instructions to the user.
pub fn print_manual_guidance() {
    crate::platform::CURRENT.print_manual_guidance();
}

/// Install udev rules for `MediaTek` preloader devices.
pub fn install_udev_rules() -> bool {
    crate::platform::CURRENT.install_udev_rules()
}

/// Add the current user to dialout/plugdev groups.
pub fn add_user_to_group() -> bool {
    crate::platform::CURRENT.add_user_to_group()
}
