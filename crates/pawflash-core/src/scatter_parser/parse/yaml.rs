use std::collections::BTreeMap;
use serde_json::{Map, Value};
use super::{ParsedRawScatter, find_general_value};

/// Parse an MTK YAML scatter into raw general metadata + partition layouts.
///
/// Real SP-Flash-Tool YAML files nest partitions under a `storage_type`
/// mapping (or a sequence of `{storage_type, description}` records), e.g.:
///
/// ```yaml
/// general:
///   platform: MT6789
///   project: DEMO
/// storage_type:
///   - storage_type: EMMC
///     description:
///       - partition_index: SYS0
///         partition_name: preloader
///         is_download: true
///         linear_start_addr: 0x0
///         partition_size: 0x800000
///       # ...
/// ```
///
/// Some files instead use a flat `- key: value` record stream. Both shapes
/// are handled by a recursive walk that groups any record carrying a
/// `partition_name`/`partition_index` under the nearest enclosing
/// `storage_type`/`layout`/`storage` value, and collects `general`-style
/// keys into the general map.
pub(crate) fn parse_yaml_scatter(text: &str) -> ParsedRawScatter {
    let mut general = Map::new();
    let mut layouts: BTreeMap<String, Vec<Map<String, Value>>> = BTreeMap::new();

    // Prefer a real YAML parse; fall back to the loose flat-record parser for
    // fragments that are not valid YAML (kept for robustness).
    match serde_yaml::from_str::<serde_yaml::Value>(text) {
        Ok(value) => walk_yaml(&value, "", &mut general, &mut layouts),
        Err(_) => {
            for rec in loose_yaml_records(text) {
                collect_json_mapping(&rec, "", &mut general, &mut layouts);
            }
        }
    }

    let general_value = Value::Object(general);
    let platform = find_general_value(&general_value, "platform");
    let project = find_general_value(&general_value, "project");

    ParsedRawScatter {
        general: general_value,
        layouts,
        platform,
        project,
        format: "yaml".to_string(),
    }
}

fn walk_yaml(
    value: &serde_yaml::Value,
    layout_hint: &str,
    general: &mut Map<String, Value>,
    layouts: &mut BTreeMap<String, Vec<Map<String, Value>>>,
) {
    match value {
        serde_yaml::Value::Mapping(map) => collect_mapping(map, layout_hint, general, layouts),
        serde_yaml::Value::Sequence(items) => {
            for item in items {
                walk_yaml(item, layout_hint, general, layouts);
            }
        }
        _ => {}
    }
}

fn collect_mapping(
    map: &serde_yaml::Mapping,
    layout_hint: &str,
    general: &mut Map<String, Value>,
    layouts: &mut BTreeMap<String, Vec<Map<String, Value>>>,
) {
    let mut is_general = false;
    let mut is_partition = false;
    let mut this_layout = layout_hint.to_string();

    for (k, v) in map {
        let key = k.as_str().unwrap_or("");
        match key {
            "general" | "config_version" | "platform" | "project" => is_general = true,
            "partition_name" | "partition_index" => is_partition = true,
            "storage_type" | "layout" | "storage" => {
                if let Some(s) = v.as_str() {
                    if !s.trim().is_empty() {
                        this_layout = s.trim().to_string();
                    }
                }
            }
            _ => {}
        }
    }

    if is_general && !is_partition {
        for (k, v) in map {
            if let Some(key) = k.as_str() {
                if !matches!(key, "general" | "storage_type" | "layout" | "storage") {
                    general
                        .entry(key.to_string())
                        .or_insert(yaml_to_json(v));
                }
            }
        }
    }

    if is_partition {
        let layout = if this_layout.trim().is_empty() {
            "DEFAULT".to_string()
        } else {
            this_layout.clone()
        };
        layouts
            .entry(layout)
            .or_default()
            .push(yaml_map_to_json(map));
    }

    for (k, v) in map {
        let _ = k;
        walk_yaml(v, &this_layout, general, layouts);
    }
}

/// Same classification as [`collect_mapping`] but for the flat JSON records
/// produced by the loose fallback parser.
fn collect_json_mapping(
    map: &Map<String, Value>,
    layout_hint: &str,
    general: &mut Map<String, Value>,
    layouts: &mut BTreeMap<String, Vec<Map<String, Value>>>,
) {
    let mut is_general = false;
    let mut is_partition = false;
    let mut this_layout = layout_hint.to_string();

    for (key, value) in map {
        match key.as_str() {
            "general" | "config_version" | "platform" | "project" => is_general = true,
            "partition_name" | "partition_index" => is_partition = true,
            "storage_type" | "layout" | "storage" => {
                if let Some(s) = value.as_str() {
                    if !s.trim().is_empty() {
                        this_layout = s.trim().to_string();
                    }
                }
            }
            _ => {}
        }
    }

    if is_general && !is_partition {
        for (key, value) in map {
            if !matches!(key.as_str(), "general" | "storage_type" | "layout" | "storage") {
                general.entry(key.clone()).or_insert_with(|| value.clone());
            }
        }
    }

    if is_partition {
        let layout = if this_layout.trim().is_empty() {
            "DEFAULT".to_string()
        } else {
            this_layout
        };
        layouts.entry(layout).or_default().push(map.clone());
    }
}

/// Convert a `serde_yaml::Mapping` into a JSON object, recursively.
fn yaml_map_to_json(map: &serde_yaml::Mapping) -> Map<String, Value> {
    let mut out = Map::new();
    for (k, v) in map {
        if let Some(key) = k.as_str() {
            out.insert(key.to_string(), yaml_to_json(v));
        }
    }
    out
}

/// Convert any `serde_yaml::Value` into a JSON value, recursively.
fn yaml_to_json(value: &serde_yaml::Value) -> Value {
    match value {
        serde_yaml::Value::Null => Value::Null,
        serde_yaml::Value::Bool(b) => Value::Bool(*b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Number(i.into())
            } else if let Some(u) = n.as_u64() {
                Value::Number(u.into())
            } else if let Some(f) = n.as_f64() {
                serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number)
            } else {
                Value::Null
            }
        }
        serde_yaml::Value::String(s) => Value::String(s.clone()),
        serde_yaml::Value::Sequence(items) => {
            Value::Array(items.iter().map(yaml_to_json).collect())
        }
        serde_yaml::Value::Mapping(m) => Value::Object(yaml_map_to_json(m)),
        serde_yaml::Value::Tagged(tagged) => yaml_to_json(&tagged.value),
    }
}

/// Fallback for non-YAML fragments: a flat `- key: value` record stream.
fn loose_yaml_records(text: &str) -> Vec<Map<String, Value>> {
    use super::scalar_json;

    let mut records = Vec::new();
    let mut current: Option<Map<String, Value>> = None;
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('-') {
            if let Some(record) = current.take().filter(|r| !r.is_empty()) {
                records.push(record);
            }
            current = Some(Map::new());
            let trimmed = rest.trim();
            if let Some((key, value)) = trimmed.split_once(':') {
                let k = key.trim();
                if !k.is_empty() {
                    if let Some(r) = current.as_mut() {
                        r.insert(k.to_string(), scalar_json(value.trim()));
                    }
                }
            }
            continue;
        }
        let Some(record) = current.as_mut() else {
            continue;
        };
        if let Some((key, value)) = line.split_once(':') {
            let k = key.trim();
            if !k.is_empty() {
                record.insert(k.to_string(), scalar_json(value.trim()));
            }
        }
    }
    if let Some(record) = current.filter(|r| !r.is_empty()) {
        records.push(record);
    }
    records
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout_names(p: &ParsedRawScatter) -> Vec<String> {
        p.layouts.keys().cloned().collect()
    }

    #[test]
    fn parse_nested_storage_type_yaml() {
        let text = r"
general:
  config_version: V1.0.3
  platform: MT6789
  project: DEMO
storage_type:
  - storage_type: EMMC
    description:
      - partition_index: SYS0
        partition_name: preloader
        is_download: true
        linear_start_addr: 0x0
        partition_size: 0x800000
      - partition_index: SYS1
        partition_name: boot
        is_download: true
  - storage_type: UFS
    description:
      - partition_index: U0
        partition_name: nvram
        is_download: false
";
        let parsed = parse_yaml_scatter(text);
        assert_eq!(parsed.platform.as_deref(), Some("MT6789"));
        assert_eq!(parsed.project.as_deref(), Some("DEMO"));
        assert_eq!(layout_names(&parsed), vec!["EMMC", "UFS"]);
        let emmc = parsed.layouts["EMMC"].clone();
        assert_eq!(emmc.len(), 2);
        assert_eq!(emmc[0].get("partition_name"), Some(&Value::String("preloader".into())));
        assert_eq!(emmc[0].get("partition_size"), Some(&Value::from(0x800_000i64)));
        let ufs = parsed.layouts["UFS"].clone();
        assert_eq!(ufs.len(), 1);
        assert_eq!(ufs[0].get("partition_name"), Some(&Value::String("nvram".into())));
    }

    #[test]
    fn parse_flat_record_yaml() {
        let text = r"
- partition_index: SYS0
  partition_name: boot
  storage: EMMC
  is_download: true
- partition_index: SYS1
  partition_name: system
  storage: EMMC
  is_download: true
";
        let parsed = parse_yaml_scatter(text);
        assert_eq!(layout_names(&parsed), vec!["EMMC"]);
        assert_eq!(parsed.layouts["EMMC"].len(), 2);
    }

    #[test]
    fn parse_yaml_value_containing_colon() {
        let text = "general:\n  platform: MT6789\n  project: a:b:c\n";
        let parsed = parse_yaml_scatter(text);
        assert_eq!(parsed.project.as_deref(), Some("a:b:c"));
    }

    #[test]
    fn loose_fallback_keeps_flat_records() {
        let parsed = parse_yaml_scatter("not: valid\n: yaml\n- partition_index: SYS0\n  partition_name: boot\n");
        assert_eq!(parsed.layouts.values().map(Vec::len).sum::<usize>(), 1);
    }
}
