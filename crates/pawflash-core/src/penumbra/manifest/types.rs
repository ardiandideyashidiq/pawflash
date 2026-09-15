//! Data structures for DA entries, blobs, and manifest catalog.

use serde::{Deserialize, Serialize};

/// Metadata for a downloadable binary blob (DA or Auth).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileBlob {
    pub url: String,
    pub sha256: String,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<u64>,
}

/// One hosted or custom DA blob and optional companion auth blob.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DAEntry {
    #[serde(default)]
    pub id: Option<String>,
    /// OEM/brand subdirectory, e.g. `infinix`.
    pub brand: String,
    /// `SoC` chipset, e.g. `mt6789`.
    pub chipset: String,
    /// Retail device names using this DA, e.g. `["Infinix NOTE 12"]`.
    #[serde(default)]
    pub devices: Vec<String>,
    /// Nested DA binary blob info.
    #[serde(default)]
    pub da: Option<FileBlob>,
    /// Optional companion auth file (e.g. for SLA/DAA secured chipsets).
    #[serde(default)]
    pub auth: Option<FileBlob>,
    /// Raw download URL for the blob (flat fallback).
    #[serde(default)]
    pub url: String,
    /// SHA-256 of the blob (flat fallback).
    #[serde(default)]
    pub sha256: String,
    /// Optional raw download URL for auth file (flat fallback).
    #[serde(default)]
    pub auth_url: Option<String>,
    /// Optional SHA-256 for auth file (flat fallback).
    #[serde(default)]
    pub auth_sha256: Option<String>,
    /// Whether verified working by community.
    #[serde(default)]
    pub verified: Option<bool>,
    /// Optional notes / instructions.
    #[serde(default)]
    pub notes: Option<String>,
    /// Whether this entry is user-defined / custom added.
    #[serde(default)]
    pub is_custom: Option<bool>,
}

impl DAEntry {
    /// Resolved DA download URL.
    #[must_use]
    pub fn da_url(&self) -> &str {
        if let Some(ref d) = self.da {
            &d.url
        } else {
            &self.url
        }
    }

    /// Resolved DA SHA-256 digest.
    #[must_use]
    pub fn da_sha256(&self) -> &str {
        if let Some(ref d) = self.da {
            &d.sha256
        } else {
            &self.sha256
        }
    }

    /// Resolved companion Auth download URL if present.
    #[must_use]
    pub fn auth_url(&self) -> Option<&str> {
        if let Some(ref a) = self.auth {
            Some(&a.url)
        } else {
            self.auth_url.as_deref()
        }
    }

    /// Resolved companion Auth SHA-256 digest if present.
    #[must_use]
    pub fn auth_sha256(&self) -> Option<&str> {
        if let Some(ref a) = self.auth {
            Some(&a.sha256)
        } else {
            self.auth_sha256.as_deref()
        }
    }

    /// Returns `true` if this DA has a companion Auth file.
    #[must_use]
    pub const fn has_auth(&self) -> bool {
        self.auth.is_some() || self.auth_url.is_some()
    }

    /// Returns `true` if this entry is a user-added custom DA.
    #[must_use]
    pub const fn is_custom_entry(&self) -> bool {
        matches!(self.is_custom, Some(true))
    }

    /// Human-readable label for pickers: `"Infinix NOTE 12  (infinix · mt6789)"`.
    #[must_use]
    pub fn label(&self) -> String {
        let device = self.devices.first().map_or("unknown device", String::as_str);
        format!("{device}  ({brand} · {chipset})", brand = self.brand, chipset = self.chipset)
    }
}

/// The DA manifest document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DAManifest {
    pub version: String,
    #[serde(default)]
    pub dais: Vec<DAEntry>,
}
