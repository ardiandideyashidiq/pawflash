//! Device-name and chipset resolution against a list of DA entries.

use crate::penumbra::manifest::types::DAEntry;
use crate::penumbra::{PenumbraError, Result};

/// Normalized lowercase form used for fuzzy matching.
pub(crate) fn normalize(s: &str) -> String {
    s.to_lowercase()
}

/// Resolve a DA by retail device name over an explicit entry list.
///
/// Matching is case-insensitive substring scoring over `devices` (primary)
/// and `(brand, chipset)` (fallback).
///
/// # Errors
///
/// Returns [`PenumbraError::NoSuchDa`] with close-match hints if no match is found.
pub fn resolve_by_device_in(dais: &[DAEntry], query: &str) -> Result<DAEntry> {
    let q = normalize(query);
    let q_tokens: Vec<&str> = q.split_whitespace().collect();

    let mut best: Option<(usize, &DAEntry)> = None;
    for entry in dais {
        let mut score = 0;
        let mut candidates: Vec<&str> =
            entry.devices.iter().map(String::as_str).collect();
        candidates.extend([entry.brand.as_str(), entry.chipset.as_str()]);
        for name in candidates {
            let n = normalize(name);
            if n == q {
                score = 1000;
            } else if n.contains(&q) {
                score = score.max(500);
            } else if q_tokens.iter().all(|t| n.contains(t)) {
                score = score.max(300);
            }
        }
        if let Some((best_score, _)) = best {
            if score > best_score {
                best = Some((score, entry));
            }
        } else if score > 0 {
            best = Some((score, entry));
        }
    }

    if let Some((_, entry)) = best {
        return Ok(entry.clone());
    }

    let hints: Vec<String> = dais
        .iter()
        .flat_map(|e| e.devices.iter().map(Clone::clone))
        .take(5)
        .collect();
    let hint_msg = if hints.is_empty() {
        String::new()
    } else {
        format!(" (did you mean: {})", hints.join(", "))
    };
    Err(PenumbraError::NoSuchDa { query: format!("{query}{hint_msg}") })
}

/// Resolve a DA by exact `(brand, chipset)` over an explicit entry list.
///
/// # Errors
///
/// Returns [`PenumbraError::NoSuchDa`] when no entry matches.
pub fn resolve_by_brand_chipset_in(
    dais: &[DAEntry],
    brand: &str,
    chipset: &str,
) -> Result<DAEntry> {
    dais.iter()
        .find(|e| {
            normalize(&e.brand) == normalize(brand) && normalize(&e.chipset) == normalize(chipset)
        })
        .cloned()
        .ok_or_else(|| PenumbraError::NoSuchDa {
            query: format!("{brand}/{chipset}"),
        })
}
