use crate::scatter_parser::safety::{
    ANDROID_CANONICAL, BOOTLOADER_CANONICAL, BOOT_CHAIN_CANONICAL, MCU_FW_CANONICAL,
    MODEM_CANONICAL, REGIONAL_CANONICAL,
};
use crate::scatter_parser::types::{ScatterPartition};

pub(super) fn storage_str(storage: crate::scatter_parser::types::StorageSelect) -> String {
    match storage {
        crate::scatter_parser::types::StorageSelect::Auto => "auto",
        crate::scatter_parser::types::StorageSelect::All => "all",
        crate::scatter_parser::types::StorageSelect::Ufs => "ufs",
        crate::scatter_parser::types::StorageSelect::Emmc => "emmc",
    }
    .to_string()
}

pub(super) fn full_flash_allows_partition(
    part: &ScatterPartition,
    image_source: &ScatterPartition,
    include_preloader: bool,
) -> (bool, String) {
    let canonical = part.canonical();
    let safety = part.safety_class();
    // Use the image-presence predicate (not `flashable_by_profile`) so a
    // partition declared with `partition_size: 0` but carrying an image is
    // still eligible; the device validates the real extent.
    let flashable = image_source.has_image();

    if matches!(safety.as_str(), "identity_or_calibration" | "dangerous") {
        return (false, format!("blocked safety class: {safety}"));
    }
    if canonical == "preloader" && !include_preloader {
        return (false, "preloader requires --include-preloader".to_string());
    }

    if !flashable {
        return (
            false,
            "not selected by scatter profile or no image".to_string(),
        );
    }
    // userdata is always flashed when it carries an image.
    if canonical == "userdata" {
        return (true, "userdata is always flashed".to_string());
    }
    if BOOTLOADER_CANONICAL.contains(&canonical.as_str())
        || BOOT_CHAIN_CANONICAL.contains(&canonical.as_str())
        || MODEM_CANONICAL.contains(&canonical.as_str())
        || MCU_FW_CANONICAL.contains(&canonical.as_str())
        || ANDROID_CANONICAL.contains(&canonical.as_str())
        || REGIONAL_CANONICAL.contains(&canonical.as_str())
    {
        (true, "allowed by full flash".to_string())
    } else {
        (false, "not included in full flash policy".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scatter_parser::types::ScatterPartition;

    fn part(name: &str, download: bool, has_file: bool, size: i64) -> ScatterPartition {
        ScatterPartition {
            source: "test".to_string(),
            layout: "EMMC".to_string(),
            index: None,
            name: name.to_string(),
            file_name: has_file.then(|| format!("{name}.img")),
            is_download: download,
            image_type: None,
            linear_start: 0,
            physical_start: 0,
            size,
            region: "EMMC".to_string(),
            storage: None,
            boundary_check: true,
            is_reserved: false,
            operation_type: None,
            is_upgradable: None,
            empty_boot_needed: None,
            combo_partsize_check: None,
            safety_class: String::new(),
            raw: serde_json::json!({}),
        }
    }

    fn allows(name: &str, include_preloader: bool) -> (bool, String) {
        let p = part(name, true, true, 0x0040_0000);
        full_flash_allows_partition(&p, &p, include_preloader)
    }

    #[test]
    fn full_flash_should_allow_boot() {
        let (allowed, reason) = allows("boot", false);
        assert!(allowed, "boot should be allowed: {reason}");
    }

    #[test]
    fn full_flash_should_block_identity_and_dangerous() {
        assert!(!allows("nvram", false).0);
        assert!(!allows("pgpt", false).0);
    }

    #[test]
    fn full_flash_should_require_include_preloader() {
        assert!(!allows("preloader", false).0);
        assert!(allows("preloader", true).0);
    }

    #[test]
    fn full_flash_should_always_allow_userdata_with_image() {
        let p = part("userdata", true, true, 0x0040_0000);
        let (allowed, reason) = full_flash_allows_partition(&p, &p, false);
        assert!(allowed, "userdata with an image must always flash: {reason}");
    }

    #[test]
    fn full_flash_should_skip_userdata_without_image() {
        let p = part("userdata", false, false, 0x0040_0000);
        let (allowed, _) = full_flash_allows_partition(&p, &p, false);
        assert!(!allowed, "image-less userdata should be skipped");
    }

    #[test]
    fn storage_str_should_round_trip() {
        assert_eq!(storage_str(crate::scatter_parser::types::StorageSelect::Auto), "auto");
        assert_eq!(storage_str(crate::scatter_parser::types::StorageSelect::All), "all");
        assert_eq!(storage_str(crate::scatter_parser::types::StorageSelect::Ufs), "ufs");
        assert_eq!(storage_str(crate::scatter_parser::types::StorageSelect::Emmc), "emmc");
    }
}
