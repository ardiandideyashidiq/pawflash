//! Bridge process runner and JSON-lines protocol parser.
//!
//! The frozen bridge is a single-shot process: one command per invocation.
//! Commands are base64-encoded JSON passed as `argv[1]` (except `selftest`,
//! which the bridge special-cases as the raw string). The bridge writes
//! JSON-lines events to stdout; we parse each line into an [`MtkEvent`] and
//! invoke a callback as they arrive.

use crate::mtk::error::MtkError;
use crate::mtk::types::{MtkCommand, MtkEvent, MtkOutcome};
use crate::mtk::Result;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;
use tracing::{debug, warn};

/// Time to wait for the bridge to emit a `result`/`error` after the last
/// event before declaring it hung.
const BRIDGE_TIMEOUT: Duration = Duration::from_secs(300);

/// Encode a command for the bridge's `argv[1]`.
fn encode_command(cmd: &MtkCommand) -> String {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    if matches!(cmd, MtkCommand::Selftest) {
        return "selftest".to_string();
    }
    let json = serde_json::to_string(cmd).expect("MtkCommand serialization cannot fail");
    STANDARD.encode(json)
}

/// Parse one JSON-lines event line into an [`MtkEvent`].
fn parse_event(line: &str) -> Result<MtkEvent> {
    let line = line.trim();
    if line.is_empty() {
        return Err(MtkError::Protocol("empty event line".into()));
    }
    serde_json::from_str(line)
        .map_err(|source| MtkError::Protocol(format!("invalid event `{line}`: {source}")))
}

/// Read stdout on a background thread so a silent bridge cannot block the
/// caller past the timeout. `Ok(Some(line))` = a line, `Ok(None)` = clean EOF,
/// `Err` = read error.
fn spawn_stdout_reader(
    stdout: std::process::ChildStdout,
) -> std::sync::mpsc::Receiver<crate::mtk::Result<Option<String>>> {
    let (tx, rx) = std::sync::mpsc::channel::<crate::mtk::Result<Option<String>>>();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = tx.send(Ok(None));
                    break;
                }
                Ok(_) => {
                    if tx.send(Ok(Some(std::mem::take(&mut line)))).is_err() {
                        break;
                    }
                }
                Err(source) => {
                    let _ = tx.send(Err(MtkError::Protocol(source.to_string())));
                    break;
                }
            }
        }
    });
    rx
}

/// Apply one event to `outcome`; returns `true` when the run is finished.
fn apply_event(
    event: &MtkEvent,
    outcome: &mut MtkOutcome,
    child: &mut std::process::Child,
) -> Result<bool> {
    match event {
        MtkEvent::Result { ok, detail, bytes } => {
            outcome.ok = *ok;
            outcome.detail.clone_from(detail);
            outcome.bytes = *bytes;
            Ok(true)
        }
        MtkEvent::Error { message } => {
            let _ = child.kill();
            Err(MtkError::Bridge(message.clone()))
        }
        MtkEvent::Progress { bytes } => {
            outcome.bytes = Some(*bytes);
            Ok(false)
        }
        _ => Ok(false),
    }
}

/// Run one command against the bridge binary, using a custom timeout.
fn run_bridge_with_timeout(
    bin: &Path,
    cmd: &MtkCommand,
    timeout: Duration,
    on_event: &mut dyn FnMut(&MtkEvent),
) -> Result<MtkOutcome> {
    let argv1 = encode_command(cmd);

    let mut child = Command::new(bin)
        .arg(&argv1)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| MtkError::Spawn { bin: bin.display().to_string(), source })?;

    let stdout = child.stdout.take().expect("stdout piped");
    let rx = spawn_stdout_reader(stdout);

    let mut outcome = MtkOutcome { ok: false, detail: None, bytes: None };
    let mut finished = false;

    loop {
        match rx.recv_timeout(timeout) {
            Ok(Ok(Some(line))) => {
                let event = parse_event(&line)?;
                on_event(&event);
                if apply_event(&event, &mut outcome, &mut child)? {
                    finished = true;
                    break;
                }
            }
            Ok(Ok(None)) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                // Clean EOF or reader death (bridge exited) without a result.
                break;
            }
            Ok(Err(err)) => {
                let _ = child.kill();
                return Err(err);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(MtkError::Timeout);
            }
        }
    }

    // Drain stderr so the bridge doesn't deadlock writing to a full pipe.
    let mut stderr = String::new();
    if let Some(mut e) = child.stderr.take() {
        use std::io::Read;
        let _ = e.read_to_string(&mut stderr);
    }

    let status = child.wait().map_err(|source| MtkError::Protocol(source.to_string()))?;

    if !finished && !status.success() {
        let detail = stderr.trim();
        return Err(MtkError::Bridge(if detail.is_empty() {
            "bridge exited with nonzero status".into()
        } else {
            detail.to_string()
        }));
    }

    if !finished {
        debug!(bin = %bin.display(), "bridge exited without a result event");
        return Err(MtkError::Protocol("bridge exited without a result event".into()));
    }

    if !stderr.trim().is_empty() {
        warn!(bin = %bin.display(), stderr = %stderr.trim(), "bridge wrote to stderr");
        on_event(&MtkEvent::Log { level: "warn".into(), message: stderr.trim().into() });
    }

    if outcome.ok {
        Ok(outcome)
    } else {
        Err(MtkError::Bridge(
            outcome.detail.unwrap_or_else(|| "bridge operation failed".into()),
        ))
    }
}

/// Run one command against the bridge binary.
///
/// Lines that fail to parse as an event are treated as protocol errors (the
/// bridge always emits well-formed JSON-lines).
///
/// # Errors
///
/// Returns [`MtkError::Spawn`] if the process cannot start, [`MtkError::Timeout`]
/// if it exceeds [`BRIDGE_TIMEOUT`], [`MtkError::Bridge`] if the bridge emits
/// an `error` event or exits nonzero, or [`MtkError::Protocol`] on malformed
/// output.
pub fn run_bridge(
    bin: &Path,
    cmd: &MtkCommand,
    on_event: &mut dyn FnMut(&MtkEvent),
) -> Result<MtkOutcome> {
    run_bridge_with_timeout(bin, cmd, BRIDGE_TIMEOUT, on_event)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};
    use std::time::Instant;

    /// Serializes fake-bridge tests: writing an executable script and
    /// immediately exec'ing it races across parallel test threads (ETXTBSY),
    /// and a hung bridge child can outlive its test. Running these tests one
    /// at a time eliminates both.
    fn bridge_test_serial() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// A fake bridge script whose temp dir lives as long as this struct.
    struct FakeBridge {
        bin: std::path::PathBuf,
        _dir: tempfile::TempDir,
    }

    /// Write a fake bridge script to a temp dir.
    fn fake_bridge(body: &str) -> FakeBridge {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fakebridge");
        let script = format!("#!/bin/sh\n{body}");
        std::fs::write(&path, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).unwrap();
        }
        FakeBridge { bin: path, _dir: dir }
    }

    #[test]
    fn parses_success_events() {
        let _guard = bridge_test_serial();
        let fb = fake_bridge(
            r#"echo '{"type":"start","total":100,"partition":"boot"}'
               echo '{"type":"progress","bytes":50}'
               echo '{"type":"progress","bytes":100}'
               echo '{"type":"result","ok":true,"detail":"done","bytes":100}'
               exit 0"#,
        );
        let mut events = Vec::new();
        let outcome =
            run_bridge(&fb.bin, &MtkCommand::Selftest, &mut |e| events.push(e.clone())).unwrap();
        assert!(outcome.ok);
        assert_eq!(outcome.bytes, Some(100));
        assert_eq!(outcome.detail.as_deref(), Some("done"));
        assert_eq!(events.len(), 4);
        assert!(matches!(events[0], MtkEvent::Start { total: 100, .. }));
    }

    #[test]
    fn error_event_maps_to_err() {
        let _guard = bridge_test_serial();
        let fb = fake_bridge(
            r#"echo '{"type":"error","message":"no device"}'
               exit 1"#,
        );
        let err = run_bridge(&fb.bin, &MtkCommand::Selftest, &mut |_| {}).unwrap_err();
        assert!(matches!(err, MtkError::Bridge(msg) if msg == "no device"));
    }

    #[test]
    fn nonzero_exit_without_result_errors() {
        let _guard = bridge_test_serial();
        let fb = fake_bridge("echo 'oops' >&2; exit 2");
        let err = run_bridge(&fb.bin, &MtkCommand::Selftest, &mut |_| {}).unwrap_err();
        assert!(matches!(err, MtkError::Bridge(_)));
    }

    #[test]
    fn selftest_passes_raw_string() {
        let _guard = bridge_test_serial();
        let fb = fake_bridge(
            r#"if [ "$1" = "selftest" ]; then
                 echo '{"type":"result","ok":true,"detail":"selftest ok"}'
                 exit 0
               else
                 echo '{"type":"error","message":"wrong arg '$1'"}'
                 exit 1
               fi"#,
        );
        let outcome = run_bridge(&fb.bin, &MtkCommand::Selftest, &mut |_| {}).unwrap();
        assert!(outcome.ok);
        assert_eq!(outcome.detail.as_deref(), Some("selftest ok"));
    }

    #[test]
    fn read_command_is_base64_encoded() {
        let _guard = bridge_test_serial();
        let fb = fake_bridge(
            r#"echo "$1" | base64 -d > /dev/null 2>&1 || { echo '{"type":"error","message":"bad b64"}'; exit 1; }
               echo '{"type":"result","ok":true}'"#,
        );
        let cmd = MtkCommand::Read {
            partition: "boot".into(),
            file: "/tmp/boot.img".into(),
            parttype: crate::mtk::PartType::User,
        };
        let outcome = run_bridge(&fb.bin, &cmd, &mut |_| {}).unwrap();
        assert!(outcome.ok);
    }

    #[test]
    fn malformed_line_is_protocol_error() {
        let _guard = bridge_test_serial();
        let fb = fake_bridge("echo 'not json'; exit 1");
        let err = run_bridge(&fb.bin, &MtkCommand::Selftest, &mut |_| {}).unwrap_err();
        assert!(matches!(err, MtkError::Protocol(_)));
    }

    #[test]
    fn encode_command_round_trip() {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;

        let cmd =
            MtkCommand::Erase { partition: "boot".into(), parttype: crate::mtk::PartType::User };
        let encoded = encode_command(&cmd);
        let decoded = STANDARD.decode(&encoded).unwrap();
        let back: MtkCommand = serde_json::from_slice(&decoded).unwrap();
        assert_eq!(back, cmd);
    }

    #[test]
    fn timeout_fires_on_silent_bridge() {
        let _guard = bridge_test_serial();
        // Emits a start event then blocks on a pipe we never feed — the
        // `exec` replaces the shell so killing it leaves no orphan children
        // (which would otherwise trip ETXTBSY races in parallel test runs).
        let fb = fake_bridge(
            "echo '{\"type\":\"start\",\"total\":100,\"partition\":\"boot\"}'; exec sleep 30",
        );
        let start = Instant::now();
        let err = run_bridge_with_timeout(
            &fb.bin,
            &MtkCommand::Selftest,
            Duration::from_millis(200),
            &mut |_| {},
        )
        .unwrap_err();
        assert!(matches!(err, MtkError::Timeout));
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}
