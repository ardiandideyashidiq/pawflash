//! MediaTek scatter file parsing (XML and YAML formats).

mod helpers;
mod normalize;
mod xml;
mod yaml;

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::Path;

use encoding_rs::{UTF_16BE, UTF_16LE};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tracing::debug;

use miette::{NamedSource, SourceSpan};

use crate::scatter_parser::error::{Error, Result};
use crate::scatter_parser::types::{ScatterFile, ScatterPartition};

use normalize::{normalize_partition, validate_layouts};

// --- Re-exports ---

pub use helpers::{human_size, parse_int};
pub(crate) use helpers::{find_general_value, scalar_json, value_to_string};

/// Parse a `MediaTek` scatter file (auto-detects XML vs YAML).
///
/// # Errors
///
/// Returns [`Error::NotFile`] for non-file paths,
/// [`Error::Io`] for I/O failures,
/// [`Error::Xml`] or [`Error::Yaml`] for parse failures.
pub fn parse_scatter(path: impl AsRef<Path>) -> Result<ScatterFile> {
    let path = path.as_ref();
    if !path.is_file() {
        return Err(Error::NotFile(path.to_path_buf()));
    }
    debug!(?path, "starting scatter parse");

    let text = decode_text(path)?;
    let text_hash = sha256_text(&text);
    let mut warnings: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    let is_xml = looks_like_xml(&text);
    debug!(?path, is_xml, "scatter format detected");

    let parsed = if is_xml {
        match xml::parse_xml_scatter(&text) {
            Ok(r) => r,
            Err((detail, offset)) => {
                return Err(Error::Xml {
                    detail,
                    source_text: NamedSource::new(
                        path.display().to_string(),
                        text.clone(),
                    ),
                    span: SourceSpan::new(offset.into(), 0),
                });
            }
        }
    } else {
        yaml::parse_yaml_scatter(&text)
    };

    let mut layouts: BTreeMap<String, Vec<ScatterPartition>> = BTreeMap::new();
    for (layout, entries) in parsed.layouts {
        let norm_layout = if layout.trim().is_empty() {
            "DEFAULT".to_string()
        } else {
            layout.trim().to_string()
        };
        let mut parts = Vec::new();
        for entry in entries {
            match normalize_partition(path, &norm_layout, entry) {
                Ok(part) => parts.push(part),
                Err(err) => errors.push(format!(
                    "{norm_layout}: failed to normalize partition: {err}"
                )),
            }
        }
        layouts.insert(norm_layout, parts);
    }

    validate_layouts(&layouts, &mut warnings, &mut errors);

    Ok(ScatterFile {
        path: path.to_path_buf(),
        format: parsed.format,
        text_hash,
        platform: parsed.platform,
        project: parsed.project,
        general: parsed.general,
        layouts,
        warnings,
        errors,
    })
}

// Intermediate representation used only during parsing; fields are destructured directly.
pub(crate) struct ParsedRawScatter {
    general: Value,
    layouts: BTreeMap<String, Vec<Map<String, Value>>>,
    platform: Option<String>,
    project: Option<String>,
    format: String,
}

fn sha256_text(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn decode_text(path: &Path) -> Result<String> {
    let raw = fs::read(path)?;

    // Sniff the encoding once instead of decoding into four full copies of
    // the file and picking the least-NUL result.
    let text = if raw.starts_with(&[0xFF, 0xFE]) {
        UTF_16LE.decode(&raw[2..]).0.into_owned()
    } else if raw.starts_with(&[0xFE, 0xFF]) {
        UTF_16BE.decode(&raw[2..]).0.into_owned()
    } else {
        // A NUL byte is essentially never present in ASCII/UTF-8 scatter
        // text but appears in ~half of the bytes of UTF-16 ASCII text. UTF-16
        // ASCII is exactly 50% NULs (not a majority), so we key off a
        // decisive byte-order signal instead.
        let nul_count = raw.iter().fold(0usize, |acc, &b| acc + usize::from(b == 0));
        if nul_count > 0 {
            // Byte-order sniff over the first 256 bytes: in UTF-16LE ASCII,
            // NUL bytes sit at ODD indices (a\0b\0...); in UTF-16BE they sit
            // at EVEN indices. A 3:1 imbalance is decisive; dense but
            // balanced NULs fall back to LE (the common case).
            let (mut even_nul, mut odd_nul) = (0usize, 0usize);
            for (i, &b) in raw.iter().take(256).enumerate() {
                if b == 0 {
                    if i % 2 == 0 {
                        even_nul += 1;
                    } else {
                        odd_nul += 1;
                    }
                }
            }
            if even_nul > odd_nul * 3 {
                UTF_16BE.decode(&raw).0.into_owned()
            } else if odd_nul > even_nul * 3 || nul_count * 2 >= raw.len() {
                UTF_16LE.decode(&raw).0.into_owned()
            } else {
                String::from_utf8_lossy(&raw).into_owned()
            }
        } else {
            String::from_utf8_lossy(&raw).into_owned()
        }
    };

    Ok(normalize_newlines(&text))
}

/// Single-pass `\r\n`/`\r` → `\n` normalization (avoids two full-string
/// `replace` passes over potentially megabytes of text).
fn normalize_newlines(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out
}

fn looks_like_xml(text: &str) -> bool {
    let trimmed = text.trim_start_matches(['\u{feff}', '\n', '\r', '\t', ' ']);
    let bytes = trimmed.as_bytes();
    let len = bytes.len().min(300);
    // `<scatter` is 8 bytes — the slice must be sized to the pattern or the
    // comparison can never match.
    (len >= 8 && bytes[..8].eq_ignore_ascii_case(b"<scatter"))
        || (len >= 5 && (bytes[..5].eq_ignore_ascii_case(b"<?xml") || bytes[..5].eq_ignore_ascii_case(b"<root")))
        || (len >= 3 && bytes[..3].eq_ignore_ascii_case(b"<da"))
}

/// Detect the kind of image by magic bytes.
#[must_use]
pub fn image_magic(path: &Path) -> Option<Value> {
    let mut file = fs::File::open(path).ok()?;
    let mut head = vec![0; 8192];
    let read = file.read(&mut head).ok()?;
    head.truncate(read);
    if head.is_empty() {
        return Some(json!({"kind": "empty"}));
    }
    let kind = if head.starts_with(b"ANDROID!") {
        "android_boot_or_recovery_image"
    } else if head.starts_with(b"AVB0") {
        "android_vbmeta_image"
    } else if head.get(..4) == Some(b"\x3a\xff\x26\xed") {
        "android_sparse_image"
    } else if head.starts_with(b"ELF") || head.starts_with(b"\x7fELF") {
        "elf"
    } else if head.len() >= 0x43a
        && matches!(&head[0x438..0x43a], b"\x53\xef" | b"\xef\x53")
    {
        "possible_ext_filesystem"
    } else if head
        .get(..1024)
        .is_some_and(|bytes| bytes.windows(8).any(|w| w == b"EFI PART"))
    {
        "gpt_or_disk_image"
    } else {
        "raw_or_unknown"
    };
    Some(json!({"kind": kind}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_like_xml_detects_scatter_root() {
        assert!(looks_like_xml("<scatter>"));
        assert!(looks_like_xml("<Scatter foo=\"bar\">"));
        assert!(looks_like_xml("  <scatter>"));
        assert!(looks_like_xml("<?xml version=\"1.0\"?><scatter>"));
        assert!(looks_like_xml("<root>"));
        assert!(looks_like_xml("<da>"));
        assert!(!looks_like_xml("scatter:"));
        assert!(!looks_like_xml("yaml content here"));
    }

    #[test]
    fn parse_scatter_rejects_non_file() {
        let result = parse_scatter("/nonexistent/scatter.txt");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a file"));
    }

    #[test]
    fn decode_text_should_detect_utf16be_without_bom_any_leading_char() {
        // "<scatter>" as UTF-16BE without BOM. The first UTF-16 code unit is
        // "<" (0x3C), which the old "first char must be 'a'" probe misread as
        // UTF-16LE. The NUL-position heuristic must recover the text.
        let text = "<scatter>\n</scatter>";
        let be: Vec<u8> = text
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect();
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("scatter.txt");
        std::fs::write(&path, &be).unwrap();
        let decoded = decode_text(&path).expect("decodes as UTF-16BE");
        assert_eq!(decoded, text, "BE content must round-trip");
    }
}
