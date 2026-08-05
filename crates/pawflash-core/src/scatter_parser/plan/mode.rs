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
    clean: bool,
) -> (bool, String) {
    let canonical = part.canonical();
    let safety = part.safety_class();
    let flashable = image_source.flashable_by_profile() && part.size > 0;

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
    if clean && canonical == "userdata" {
        return (true, "allowed by --clean".to_string());
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
            raw: serde_json::json!({}),
            unknown_fields: std::collections::BTreeMap::new(),
        }
    }

    fn allows(name: &str, include_preloader: bool, clean: bool) -> (bool, String) {
        let p = part(name, true, true, 0x0040_0000);
        full_flash_allows_partition(&p, &p, include_preloader, clean)
    }

    #[test]
    fn full_flash_should_allow_boot() {
        let (allowed, reason) = allows("boot", false, false);
        assert!(allowed, "boot should be allowed: {reason}");
    }

    #[test]
    fn full_flash_should_block_identity_and_dangerous() {
        assert!(!allows("nvram", false, false).0);
        assert!(!allows("pgpt", false, false).0);
    }

    #[test]
    fn full_flash_should_require_include_preloader() {
        assert!(!allows("preloader", false, false).0);
        assert!(allows("preloader", true, false).0);
    }

    #[test]
    fn full_flash_should_allow_userdata_only_with_clean() {
        let p = part("userdata", true, true, 0x0040_0000);
        let (a1, _) = full_flash_allows_partition(&p, &p, false, true);
        assert!(a1);
        let (a2, r2) = full_flash_allows_partition(&p, &p, false, false);
        assert!(!a2, "userdata without clean should be blocked: {r2}");
    }

    #[test]
    fn storage_str_should_round_trip() {
        assert_eq!(storage_str(crate::scatter_parser::types::StorageSelect::Auto), "auto");
        assert_eq!(storage_str(crate::scatter_parser::types::StorageSelect::All), "all");
        assert_eq!(storage_str(crate::scatter_parser::types::StorageSelect::Ufs), "ufs");
        assert_eq!(storage_str(crate::scatter_parser::types::StorageSelect::Emmc), "emmc");
    }
}
