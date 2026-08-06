use std::collections::BTreeMap;

use crate::scatter_parser::types::{ScatterFile, ScatterPartition, StorageSelect};

pub(super) fn selected_partitions(
    scatter: &ScatterFile,
    storage: StorageSelect,
) -> Vec<&ScatterPartition> {
    selected_layouts(scatter, storage)
        .into_values()
        .flatten()
        .collect()
}

pub(super) fn selected_layouts(
    scatter: &ScatterFile,
    storage: StorageSelect,
) -> BTreeMap<String, Vec<&ScatterPartition>> {
    if storage == StorageSelect::All {
        return scatter
            .layouts
            .iter()
            .map(|(key, parts)| (key.clone(), parts.iter().collect()))
            .collect();
    }

    let upper_to_key = scatter
        .layouts
        .keys()
        .map(|key| (key.to_uppercase(), key))
        .collect::<BTreeMap<_, _>>();

    match storage {
        StorageSelect::Ufs => upper_to_key
            .get("UFS")
            .map(|key| BTreeMap::from([((*key).clone(), scatter.layouts[*key].iter().collect())]))
            .unwrap_or_default(),
        StorageSelect::Emmc => upper_to_key
            .get("EMMC")
            .map(|key| BTreeMap::from([((*key).clone(), scatter.layouts[*key].iter().collect())]))
            .unwrap_or_default(),
        StorageSelect::Auto => {
            for wanted in ["UFS", "EMMC"] {
                if let Some(key) = upper_to_key.get(wanted) {
                    return BTreeMap::from([((*key).clone(), scatter.layouts[*key].iter().collect())]);
                }
            }
            scatter
                .layouts
                .iter()
                .next()
                .map(|(key, parts)| (key.clone(), parts.iter().collect()))
                .into_iter()
                .collect()
        }
        StorageSelect::All => unreachable!("handled by early return"),
    }
}

pub(super) fn selected_layout_names(
    scatter: &ScatterFile,
    storage: StorageSelect,
) -> Vec<String> {
    selected_layouts(scatter, storage)
        .keys()
        .cloned()
        .collect()
}
