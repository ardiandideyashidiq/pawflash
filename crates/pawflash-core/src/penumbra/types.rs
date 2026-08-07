//! Serde event types for the penumbra DA integration.
//!
//! Unlike the mtk bridge (JSON-lines over stdout), penumbra reports progress
//! through native `FnMut(usize, usize)` callbacks. Ops wrap those callbacks
//! into a [`PenumbraEvent`] stream so the CLI and GUI can consume a uniform
//! event shape.

use serde::{Deserialize, Serialize};

/// One event emitted by a penumbra DA operation.
///
/// `Progress` carries cumulative `bytes` and a `total` that may be `0` when
/// unknown (e.g. indeterminate transfer sizes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum PenumbraEvent {
    /// Phase transition (e.g. `connect`, `handshake`, `erase`, `flash`).
    Phase { phase: String, message: String },
    /// Cumulative bytes transferred.
    Progress { bytes: u64, total: u64 },
    /// Debug/info log line.
    Log { level: String, message: String },
    /// Operation completed.
    Done { ok: bool, detail: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_event_round_trips() {
        let ev = PenumbraEvent::Phase { phase: "connect".into(), message: "waiting".into() };
        let s = serde_json::to_string(&ev).unwrap();
        let back: PenumbraEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(back, ev);
        assert!(s.contains("\"event\":\"phase\""));
    }

    #[test]
    fn progress_event_round_trips() {
        let ev = PenumbraEvent::Progress { bytes: 1_048_576, total: 134_217_728 };
        let s = serde_json::to_string(&ev).unwrap();
        let back: PenumbraEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(back, ev);
    }

    #[test]
    fn log_event_round_trips() {
        let ev = PenumbraEvent::Log { level: "info".into(), message: "handshake ok".into() };
        let s = serde_json::to_string(&ev).unwrap();
        let back: PenumbraEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(back, ev);
    }

    #[test]
    fn done_event_round_trips() {
        let ev = PenumbraEvent::Done { ok: true, detail: "read complete".into() };
        let s = serde_json::to_string(&ev).unwrap();
        let back: PenumbraEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(back, ev);
    }
}
