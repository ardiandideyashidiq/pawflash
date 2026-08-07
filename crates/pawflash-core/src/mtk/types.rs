//! Serde types mirroring the mtk bridge protocol.
//!
//! The bridge reads a single base64-encoded JSON command from `argv[1]` and
//! writes JSON-lines events to stdout. These types are the Rust-side contract:
//! [`MtkCommand`] is what we send, [`MtkEvent`] is what we parse back.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A command sent to the bridge. Serialized to JSON, then base64-encoded onto
/// `argv[1]` — except [`Self::Selftest`], which the bridge special-cases as the
/// raw string `selftest` on `argv[1]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "lowercase")]
pub enum MtkCommand {
    /// Loader-check self test; no device access.
    Selftest,
    /// Read a partition to `file`.
    Read {
        partition: String,
        file: String,
        #[serde(default)]
        parttype: PartType,
    },
    /// Write `file` to a partition.
    Write {
        partition: String,
        file: String,
        #[serde(default)]
        parttype: PartType,
    },
    /// Erase a partition.
    Erase {
        partition: String,
        #[serde(default)]
        parttype: PartType,
    },
}

/// One JSON-lines event emitted by the bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MtkEvent {
    /// Operation started; `total` is the partition length in bytes.
    #[serde(rename = "start")]
    Start { total: u64, partition: String },
    /// Phase transition (e.g. `connect`, `handshake`, `erase`, `flash`).
    #[serde(rename = "phase")]
    Phase { phase: String, message: String },
    /// Cumulative bytes transferred (guiprogress semantics).
    #[serde(rename = "progress")]
    Progress { bytes: u64 },
    /// Bridge log line (mtkclient console output re-emitted).
    #[serde(rename = "log")]
    Log { level: String, message: String },
    /// Operation completed. `bytes` present for read/write.
    #[serde(rename = "result")]
    Result { ok: bool, detail: Option<String>, bytes: Option<u64> },
    /// Fatal error event.
    #[serde(rename = "error")]
    Error { message: String },
}

/// Storage partition type selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PartType {
    #[default]
    #[serde(rename = "user")]
    User,
    #[serde(rename = "boot1")]
    Boot1,
    #[serde(rename = "boot2")]
    Boot2,
    #[serde(rename = "rpmb")]
    Rpmb,
}

impl fmt::Display for PartType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::User => "user",
            Self::Boot1 => "boot1",
            Self::Boot2 => "boot2",
            Self::Rpmb => "rpmb",
        };
        f.write_str(s)
    }
}

/// Outcome of a bridge run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtkOutcome {
    pub ok: bool,
    pub detail: Option<String>,
    /// Bytes transferred, for read/write.
    pub bytes: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_type_default_is_user() {
        assert_eq!(PartType::default(), PartType::User);
    }

    #[test]
    fn part_type_display_matches_bridge_keys() {
        assert_eq!(PartType::User.to_string(), "user");
        assert_eq!(PartType::Boot1.to_string(), "boot1");
        assert_eq!(PartType::Boot2.to_string(), "boot2");
        assert_eq!(PartType::Rpmb.to_string(), "rpmb");
    }

    #[test]
    fn read_command_round_trips() {
        let cmd = MtkCommand::Read {
            partition: "boot".into(),
            file: "/tmp/boot.img".into(),
            parttype: PartType::User,
        };
        let s = serde_json::to_string(&cmd).unwrap();
        let back: MtkCommand = serde_json::from_str(&s).unwrap();
        assert_eq!(back, cmd);
        assert!(s.contains("\"cmd\":\"read\""));
    }

    #[test]
    fn selftest_command_round_trips() {
        let cmd = MtkCommand::Selftest;
        let s = serde_json::to_string(&cmd).unwrap();
        assert_eq!(s, r#"{"cmd":"selftest"}"#);
        let back: MtkCommand = serde_json::from_str(&s).unwrap();
        assert_eq!(back, cmd);
    }

    #[test]
    fn parttype_defaults_when_omitted() {
        let json = r#"{"cmd":"erase","partition":"boot"}"#;
        let cmd: MtkCommand = serde_json::from_str(json).unwrap();
        assert_eq!(cmd, MtkCommand::Erase { partition: "boot".into(), parttype: PartType::User });
    }

    #[test]
    fn start_event_parses() {
        let json = r#"{"type":"start","total":134217728,"partition":"boot"}"#;
        let ev: MtkEvent = serde_json::from_str(json).unwrap();
        assert_eq!(ev, MtkEvent::Start { total: 134_217_728, partition: "boot".into() });
    }

    #[test]
    fn progress_event_parses() {
        let json = r#"{"type":"progress","bytes":1048576}"#;
        let ev: MtkEvent = serde_json::from_str(json).unwrap();
        assert_eq!(ev, MtkEvent::Progress { bytes: 1_048_576 });
    }

    #[test]
    fn result_event_parses() {
        let json = r#"{"type":"result","ok":true,"detail":"done","bytes":134217728}"#;
        let ev: MtkEvent = serde_json::from_str(json).unwrap();
        assert_eq!(
            ev,
            MtkEvent::Result { ok: true, detail: Some("done".into()), bytes: Some(134_217_728) }
        );
    }

    #[test]
    fn error_event_parses() {
        let json = r#"{"type":"error","message":"boom"}"#;
        let ev: MtkEvent = serde_json::from_str(json).unwrap();
        assert_eq!(ev, MtkEvent::Error { message: "boom".into() });
    }
}
