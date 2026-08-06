//! Flash plan builder — converts a parsed `ScatterFile` into a `FlashPlan`.

pub(crate) mod action;
pub(crate) mod image;
pub(crate) mod layout;
pub(crate) mod mode;
pub(crate) mod slot;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;
use tracing::debug;

use crate::scatter_parser::types::{
    FlashAction, FlashPlan, FlashPlanOptions, ScatterFile, ScatterPartition, SkippedPartition,
};

use self::action::{
    apply_exclude_filter, compute_image_counts, finalize_plan_summary, flash_action,
    PlanSummaryCounts, skipped_partition,
};
use self::image::{image_exists, resolve_images_for_plan};
use self::layout::{selected_layout_names, selected_partitions};
use self::mode::{full_flash_allows_partition, storage_str};
use self::slot::{
    check_incomplete_slots, expand_requested_names, inherited_action_reason,
    inherited_image_source_for_slot_b, synthesize_slot_actions_if_needed,
};

fn build_partition_actions<'a>(
    selected_parts: &'a [&'a ScatterPartition],
    options: &FlashPlanOptions,
    parts_by_name: &BTreeMap<String, &'a ScatterPartition>,
    scatter_dir: Option<&std::path::Path>,
) -> (Vec<FlashAction>, Vec<SkippedPartition>) {
    let mut actions = Vec::new();
    let mut skipped = Vec::new();

    for part in selected_parts {
        if part.slot().is_some() && !part.flashable_by_profile() {
            continue;
        }

        let part_ref = *part;
        let image_source = inherited_image_source_for_slot_b(part_ref, parts_by_name);
        let (allowed, reason) = full_flash_allows_partition(
            part_ref,
            image_source,
            options.allowance.include_preloader,
        );
        if !allowed {
            skipped.push(skipped_partition(part_ref, &reason));
            continue;
        }

        let (image, mut action_warnings) =
            resolve_images_for_plan(image_source, scatter_dir, options);
        if !image_exists(&image) {
            skipped.push(skipped_partition(part_ref, "image not found"));
            continue;
        }
        if part.size == 0 {
            action_warnings.insert(0, "partition reports zero size; flashing image anyway".to_string());
        }
        let action_reason =
            inherited_action_reason(reason, part_ref, image_source);

        actions.push(flash_action(
            "flash",
            part_ref,
            Some(image),
            &action_reason,
            action_warnings,
        ));
    }

    (actions, skipped)
}

/// Holds the working data that `finalize_plan` mutates to produce a [`FlashPlan`].
///
/// Collapsed from the previous 7-parameter signature so callers self-document
/// via field names and ordering mistakes become impossible.
pub(crate) struct PlanFinalizationContext<'a> {
    pub scatter: &'a ScatterFile,
    pub actions: Vec<FlashAction>,
    pub skipped: Vec<SkippedPartition>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub available_names: &'a BTreeSet<String>,
    pub selected_parts: &'a [&'a ScatterPartition],
}

#[must_use]
fn finalize_plan(
    ctx: &mut PlanFinalizationContext,
    options: &FlashPlanOptions,
) -> FlashPlan {
    let PlanFinalizationContext {
        scatter,
        warnings,
        errors,
        available_names,
        selected_parts,
        ..
    } = ctx;

    synthesize_slot_actions_if_needed(selected_parts, &mut ctx.actions);

    apply_exclude_filter(&mut ctx.actions, &mut ctx.skipped, warnings, &options.exclude, available_names);

    // Track which partition names the user excluded so the incomplete-slot
    // check does not treat a deliberately excluded slot as a missing one.
    let excluded_names = if options.exclude.is_empty() {
        BTreeSet::new()
    } else {
        expand_requested_names(&options.exclude, available_names)
    };

    // Surface non-download slot partitions that were never synthesized into
    // actions (their slot twin was excluded or its image is missing) instead
    // of silently dropping them from the plan.
    let action_names: BTreeSet<String> = ctx
        .actions
        .iter()
        .map(|a| a.partition.to_lowercase())
        .collect();
    for part in selected_parts.iter().copied() {
        let lowered = part.name.to_lowercase();
        if part.slot().is_some()
            && !part.flashable_by_profile()
            && !action_names.contains(&lowered)
            && !excluded_names.contains(&lowered)
        {
            ctx.skipped.push(skipped_partition(
                part,
                "non-download slot partition without a matching slot image",
            ));
        }
    }

    let incomplete_slots = check_incomplete_slots(
        selected_parts,
        &ctx.actions,
        &excluded_names,
        options.allowance.allow_incomplete_slots,
        warnings,
        errors,
    );

    let (missing_images, oversized_images, action_warning_count) =
        compute_image_counts(&ctx.actions);

    if options.image_verification.check_images && missing_images > 0 {
        errors.push(format!("missing images: {missing_images}"));
    }
    if options.image_verification.check_images && oversized_images > 0 {
        errors.push(format!("oversized images: {oversized_images}"));
    }

    debug!(
        actions = ctx.actions.len(),
        skipped = ctx.skipped.len(),
        warnings = warnings.len(),
        errors = errors.len(),
        "flash plan summary",
    );

    let summary = finalize_plan_summary(
        &ctx.actions,
        &PlanSummaryCounts {
            skipped: ctx.skipped.len(),
            missing_image: missing_images,
            oversized_image: oversized_images,
            action_warnings: action_warning_count,
            incomplete_slot_bases: incomplete_slots.len(),
            warnings: warnings.len(),
            errors: errors.len(),
        },
    );

    FlashPlan {
        storage_selection: storage_str(options.storage),
        selected_layouts: selected_layout_names(scatter, options.storage),
        platform: scatter.platform.clone(),
        project: scatter.project.clone(),
        firmware_dir: options.firmware_dir.as_ref().map(|p| p.to_string_lossy().into_owned()),
        package_root: options.package_root.as_ref().map(|p| p.to_string_lossy().into_owned()),
        options: json!({
            "check_images": options.image_verification.check_images,
            "image_search": options.image_verification.image_search,
            "include_preloader": options.allowance.include_preloader,
            "allow_incomplete_slots": options.allowance.allow_incomplete_slots,
            "exclude": options.exclude.clone(),
        }),
        summary,
        actions: core::mem::take(&mut ctx.actions),
        skipped: core::mem::take(&mut ctx.skipped),
        incomplete_slots,
        warnings: core::mem::take(warnings),
        errors: core::mem::take(errors),
    }
}

/// Build a safe flash plan for a parsed scatter file.
///
/// # Errors
///
/// Returns [`Error::InvalidValue`] if partition fields cannot be parsed.
#[must_use]
pub fn build_flash_plan(scatter: &ScatterFile, options: &FlashPlanOptions) -> FlashPlan {
    debug!(
        storage = %storage_str(options.storage),
        "building flash plan",
    );
    let warnings = Vec::new();
    let errors = Vec::new();

    let selected_parts = selected_partitions(scatter, options.storage);
    let parts_by_name = selected_parts
        .iter()
        .map(|part| (part.name.to_lowercase(), *part))
        .collect::<BTreeMap<_, _>>();
    let available_names = parts_by_name.keys().cloned().collect::<BTreeSet<_>>();

    let scatter_dir = scatter.path.parent();
    let (actions, skipped) = build_partition_actions(
        &selected_parts,
        options,
        &parts_by_name,
        scatter_dir,
    );

    let mut ctx = PlanFinalizationContext {
        scatter,
        actions,
        skipped,
        warnings,
        errors,
        available_names: &available_names,
        selected_parts: &selected_parts,
    };
    finalize_plan(&mut ctx, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scatter_parser::types::ScatterPartition;

    fn synthetic_part(name: &str, download: bool, has_file: bool, size: i64) -> ScatterPartition {
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
            region: "EMMC_BOOT1".to_string(),
            storage: None,
            boundary_check: true,
            is_reserved: false,
            operation_type: None,
            is_upgradable: None,
            empty_boot_needed: None,
            combo_partsize_check: None,
            safety_class: String::new(),
            raw: json!({}),
        }
    }

    fn userdata_part() -> ScatterPartition {
        ScatterPartition {
            name: "userdata".to_string(),
            size: 0,
            is_download: false,
            file_name: None,
            ..synthetic_part("userdata", false, false, 0)
        }
    }

    /// Write a dummy image file for every flashable partition and point the
    /// scatter path into the temp dir so image resolution succeeds.
    fn scatter_with_images(scatter: &mut ScatterFile) -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().expect("temp dir");
        for part in scatter.layouts.values().flatten() {
            if let Some(name) = part.file_name.as_deref() {
                std::fs::write(dir.path().join(name), b"dummy image").expect("write image");
            }
        }
        scatter.path = dir.path().join("test.xml");
        dir
    }

    fn synthetic_ab_scatter() -> ScatterFile {
        let mut layouts = std::collections::BTreeMap::new();
        layouts.insert(
            "EMMC".to_string(),
            vec![
                synthetic_part("boot_a", true, true, 0x0040_0000),
                synthetic_part("boot_b", false, false, 0x0040_0000),
                synthetic_part("dtbo_a", true, true, 0x0010_0000),
                synthetic_part("dtbo_b", false, false, 0x0010_0000),
                userdata_part(),
            ],
        );
        ScatterFile {
            path: std::path::PathBuf::from("test.xml"),
            format: "xml".to_string(),
            text_hash: "abc".to_string(),
            platform: Some("MT6789".to_string()),
            project: None,
            general: json!({}),
            layouts,
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }

    #[test]
    fn build_flash_plan_should_select_ufs_layout_by_default() {
        let mut layouts = std::collections::BTreeMap::new();
        layouts.insert("EMMC".to_string(), vec![]);
        layouts.insert(
            "UFS".to_string(),
            vec![synthetic_part("boot", true, true, 0x0040_0000)],
        );
        let scatter = ScatterFile {
            path: std::path::PathBuf::from("test.xml"),
            format: "xml".to_string(),
            text_hash: "abc".to_string(),
            platform: None,
            project: None,
            general: json!({}),
            layouts,
            warnings: Vec::new(),
            errors: Vec::new(),
        };

        let default_options = FlashPlanOptions::default();
        let plan = build_flash_plan(&scatter, &default_options);
        assert_eq!(plan.selected_layouts, vec!["UFS"]);
    }

    #[test]
    fn build_flash_plan_should_error_when_both_slots_are_incomplete() {
        let mut layouts = std::collections::BTreeMap::new();
        layouts.insert(
            "EMMC".to_string(),
            vec![
                synthetic_part("boot_a", true, true, 0x0040_0000),
                synthetic_part("boot_b", true, true, 0x0040_0000),
                synthetic_part("dtbo_a", true, true, 0x0010_0000),
                synthetic_part("dtbo_b", true, true, 0x0010_0000),
                userdata_part(),
            ],
        );
        let mut scatter = ScatterFile {
            path: std::path::PathBuf::from("test.xml"),
            format: "xml".to_string(),
            text_hash: "abc".to_string(),
            platform: Some("MT6789".to_string()),
            project: None,
            general: json!({}),
            layouts,
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        let dir = scatter_with_images(&mut scatter);
        // Make boot_b's image unresolvable so only boot_a is planned: with no
        // explicit exclusion this must surface as an incomplete-slot error.
        std::fs::remove_file(dir.path().join("boot_b.img")).expect("remove boot_b image");
        let options = FlashPlanOptions::default();
        let plan = build_flash_plan(&scatter, &options);
        assert!(!plan.errors.is_empty(), "expected incomplete slot errors");
        assert!(
            plan.errors.iter().any(|e| e.contains("boot")),
            "error should mention boot: {:?}",
            plan.errors
        );
    }

    #[test]
    fn build_flash_plan_should_not_flag_explicitly_excluded_slot_as_incomplete() {
        let mut scatter = synthetic_ab_scatter();
        let _dir = scatter_with_images(&mut scatter);
        let options = FlashPlanOptions {
            exclude: vec!["boot_b".to_string()],
            ..FlashPlanOptions::default()
        };
        let plan = build_flash_plan(&scatter, &options);
        assert!(plan.errors.is_empty(), "explicit exclusion must not error: {:?}", plan.errors);
        assert!(
            !plan.actions.iter().any(|a| a.partition == "boot_b"),
            "boot_b must be excluded"
        );
        assert!(
            plan.skipped.iter().any(|s| s.partition == "boot_b"),
            "excluded boot_b should be reported as skipped"
        );
    }

    #[test]
    fn build_flash_plan_should_skip_non_download_slot_without_source() {
        let mut layouts = std::collections::BTreeMap::new();
        // boot_b is non-download and boot_a is not present at all.
        layouts.insert(
            "EMMC".to_string(),
            vec![synthetic_part("boot_b", false, false, 0x0040_0000), userdata_part()],
        );
        let mut scatter = ScatterFile {
            path: std::path::PathBuf::from("test.xml"),
            format: "xml".to_string(),
            text_hash: "abc".to_string(),
            platform: Some("MT6789".to_string()),
            project: None,
            general: json!({}),
            layouts,
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        let _dir = scatter_with_images(&mut scatter);
        let options = FlashPlanOptions::default();
        let plan = build_flash_plan(&scatter, &options);
        assert!(
            plan.skipped.iter().any(|s| s.partition == "boot_b"),
            "orphan non-download slot must appear in skipped, got: {:?}",
            plan.skipped.iter().map(|s| &s.partition).collect::<Vec<_>>()
        );
        assert!(!plan.actions.iter().any(|a| a.partition == "boot_b"));
    }

    #[test]
    fn build_flash_plan_should_flash_zero_size_partition_with_image() {
        let mut layouts = std::collections::BTreeMap::new();
        layouts.insert(
            "EMMC".to_string(),
            vec![synthetic_part("boot", true, true, 0), userdata_part()],
        );
        let mut scatter = ScatterFile {
            path: std::path::PathBuf::from("test.xml"),
            format: "xml".to_string(),
            text_hash: "abc".to_string(),
            platform: Some("MT6789".to_string()),
            project: None,
            general: json!({}),
            layouts,
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        let _dir = scatter_with_images(&mut scatter);
        let plan = build_flash_plan(&scatter, &FlashPlanOptions::default());
        assert!(
            plan.actions.iter().any(|a| a.partition == "boot"),
            "zero-size partition with an image must still be flashable: {:?}",
            plan.actions.iter().map(|a| &a.partition).collect::<Vec<_>>()
        );
        let boot = plan.actions.iter().find(|a| a.partition == "boot").unwrap();
        assert!(
            boot.warnings.iter().any(|w| w.contains("zero size")),
            "expected a zero-size warning: {:?}",
            boot.warnings
        );
    }

    #[test]
    fn build_flash_plan_should_synthesize_non_download_b_slots() {
        let mut scatter = synthetic_ab_scatter();
        let _dir = scatter_with_images(&mut scatter);
        let options = FlashPlanOptions::default();
        let plan = build_flash_plan(&scatter, &options);
        let b_actions: Vec<_> = plan
            .actions
            .iter()
            .filter(|a| a.partition.ends_with("_b"))
            .collect();
        assert!(!b_actions.is_empty(), "expected synthesized slot b actions");
        assert!(
            b_actions.iter().any(|a| a.partition == "boot_b"),
            "expected boot_b: {:?}",
            b_actions.iter().map(|a| &a.partition).collect::<Vec<_>>()
        );
    }

    #[test]
    fn full_flash_should_include_userdata_with_image() {
        // synthetic_ab_scatter's userdata carries no image — build a scatter
        // whose userdata has one and assert it is now always flashed.
        let mut layouts = std::collections::BTreeMap::new();
        layouts.insert(
            "EMMC".to_string(),
            vec![
                synthetic_part("boot", true, true, 0x0040_0000),
                synthetic_part("userdata", true, true, 0x1000_0000),
            ],
        );
        let mut scatter = ScatterFile {
            path: std::path::PathBuf::from("test.xml"),
            format: "xml".to_string(),
            text_hash: "abc".to_string(),
            platform: Some("MT6789".to_string()),
            project: None,
            general: json!({}),
            layouts,
            warnings: Vec::new(),
            errors: Vec::new(),
        };
        let _dir = scatter_with_images(&mut scatter);
        let plan = build_flash_plan(&scatter, &FlashPlanOptions::default());
        assert!(
            plan.actions.iter().any(|a| a.partition == "userdata"),
            "userdata with an image must be included by default"
        );
    }

    #[test]
    fn full_flash_should_skip_userdata_without_image() {
        let mut scatter = synthetic_ab_scatter();
        let _dir = scatter_with_images(&mut scatter);
        let plan = build_flash_plan(&scatter, &FlashPlanOptions::default());
        assert!(
            !plan.actions.iter().any(|a| a.partition == "userdata"),
            "image-less userdata should be skipped"
        );
    }

    #[test]
    fn flash_plan_options_should_use_kebab_case_wire_contract() {
        let options = FlashPlanOptions::default();
        let json = serde_json::to_string(&options).expect("default options serialize");
        assert!(json.contains("\"storage\":\"auto\""), "json: {json}");

        let round_tripped: FlashPlanOptions =
            serde_json::from_str(&json).expect("kebab-case json deserializes");
        assert_eq!(round_tripped.storage, options.storage);
    }
}
