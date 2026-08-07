//! CLI integration tests — exercise the user-facing `pawflash` contract
//! (flag parsing, dry-run/JSON output, simulate execution, exit codes)
//! without any hardware. Uses `assert_cmd`/`predicates` (already declared in
//! `crates/pawflash/Cargo.toml` dev-dependencies).

use std::path::PathBuf;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

/// Absolute path to the fixture directory (resolved at compile time).
fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn scatter_path() -> PathBuf {
    fixture_dir().join("minimal.xml")
}

#[test]
fn help_exits_zero_and_lists_flash() {
    let mut cmd = Command::cargo_bin("pawflash").expect("binary resolves");
    let assert = cmd.arg("--help").assert();
    assert.success().stdout(predicate::str::contains("flash"));
}

#[test]
fn flash_scatter_dry_run_prints_plan() {
    let mut cmd = Command::cargo_bin("pawflash").expect("binary resolves");
    let assert = cmd
        .arg("flash")
        .arg("scatter")
        .arg(scatter_path())
        .arg("--dry-run")
        .assert();
    assert
        .success()
        .stdout(predicate::str::contains("Flash Plan"))
        .stdout(predicate::str::contains("boot"));
}

#[test]
fn flash_scatter_dry_run_json_output_is_valid() {
    let mut cmd = Command::cargo_bin("pawflash").expect("binary resolves");
    let output = cmd
        .arg("flash")
        .arg("scatter")
        .arg(scatter_path())
        .arg("--dry-run")
        .arg("--json")
        .output()
        .expect("command runs");
    assert!(output.status.success(), "dry-run --json must succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let plan: serde_json::Value = serde_json::from_str(stdout.trim())
        .expect("dry-run --json output must parse as JSON");
    assert_eq!(plan["summary"]["flash_count"], 1, "plan: {plan}");
    let actions = plan["actions"].as_array().expect("actions array");
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0]["partition"], "boot");
}

#[test]
fn flash_scatter_simulate_executes_headless() {
    // `--simulate --json` runs the simulated transport headlessly (no TTY
    // needed) and prints the JSON result. Bare `--simulate` enters the
    // interactive confirm flow, which requires a terminal.
    let mut cmd = Command::cargo_bin("pawflash").expect("binary resolves");
    let output = cmd
        .arg("flash")
        .arg("scatter")
        .arg(scatter_path())
        .arg("--simulate")
        .arg("--json")
        .output()
        .expect("command runs");
    assert!(output.status.success(), "simulated flash must succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let result: serde_json::Value = serde_json::from_str(stdout.trim())
        .expect("simulate --json output must parse as JSON");
    assert_eq!(result["succeeded"], 1, "result: {result}");
    assert_eq!(result["failed"], 0, "result: {result}");
    assert_eq!(result["outcomes"][0]["partition"], "boot");
}

#[test]
fn invalid_storage_option_errors() {
    let mut cmd = Command::cargo_bin("pawflash").expect("binary resolves");
    let assert = cmd
        .arg("flash")
        .arg("scatter")
        .arg("--storage")
        .arg("bogus")
        .arg(scatter_path())
        .arg("--dry-run")
        .assert();
    assert.failure();
}

#[test]
fn missing_scatter_file_errors() {
    let mut cmd = Command::cargo_bin("pawflash").expect("binary resolves");
    let assert = cmd
        .arg("flash")
        .arg("scatter")
        .arg("/nonexistent.xml")
        .arg("--dry-run")
        .assert();
    assert.failure();
}

#[test]
fn json_without_dry_run_is_refused() {
    // Plan 004: `--json` must never silently flash; it requires --dry-run.
    let mut cmd = Command::cargo_bin("pawflash").expect("binary resolves");
    let assert = cmd
        .arg("flash")
        .arg("scatter")
        .arg(scatter_path())
        .arg("--json")
        .assert();
    assert.failure();
}

#[test]
fn help_shows_flash_subcommands() {
    let mut cmd = Command::cargo_bin("pawflash").expect("binary resolves");
    let assert = cmd
        .arg("flash")
        .arg("--help")
        .assert();
    assert
        .success()
        .stdout(predicate::str::contains("scatter"))
        .stdout(predicate::str::contains("raw"));
}
