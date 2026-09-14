//! USB device detection and udev rule management for mtkclient DA devices.
//!
//! Device detection works on both Linux and Windows via `nusb`.

/// USB vendor IDs that mtkclient treats as DA-capable devices
/// (`mtkclient/config/usb_ids.py`): `MediaTek`, LG, OPPO, Sony.
pub const DEVICE_VENDOR_IDS: [u16; 4] = [0x0e8d, 0x1004, 0x22d9, 0x0fce];

/// Whether any DA-capable USB device is present.
///
/// Enumerates devices via `nusb::list_devices()` and matches against
/// [`DEVICE_VENDOR_IDS`].
pub async fn device_visible() -> bool {
    match nusb::list_devices().await {
        Ok(mut devices) => devices.any(|d| DEVICE_VENDOR_IDS.contains(&d.vendor_id())),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendor_ids_match_mtkclient() {
        assert_eq!(DEVICE_VENDOR_IDS, [0x0e8d, 0x1004, 0x22d9, 0x0fce]);
    }
}
