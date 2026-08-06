//! Tauri v2 desktop app for pawflash — exposes core flashing operations as
//! IPC commands with progress reporting via `Channel<ProgressEvent>`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use pawflash_core::flash::executor::BootTarget;
use pawflash_core::flash::progress::{FlashRunOptions, FlashTransferEvent};
use pawflash_core::flash::FlashExecutor;
use pawflash_core::scatter_parser as sp;
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{Emitter, State};
use tracing::{debug, info, trace, warn};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, registry::Registry};

// ── Logging init ──────────────────────────────────────────────────────

fn init_logging() {
  let subscriber = Registry::default()
    .with(LevelFilter::INFO)
    .with(
      fmt::Layer::new()
        .with_writer(std::io::stderr)
        .with_ansi(true)
        .with_target(true)
        .with_level(true)
        .compact(),
    );
  let _ = tracing::subscriber::set_global_default(subscriber);
}

// ── Event types ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum ProgressEvent {
  Phase { phase: String, message: String },
  FlashProgress { partition: String, percent: f64 },
  Flashing {
    partition: String,
    operation: String,
    bytes: u64,
    total: u64,
    overall_bytes: u64,
    overall_total: u64,
  },
  FlashComplete { partition: String, success: bool, response: Option<String> },

  DeviceAction { action: String, detail: String },
  Overall { bytes: u64, total: u64 },
  Warning { message: String },
  Error { message: String },
  Cancelled { message: String },
  ForceFastbootStage { stage: String, message: String },
  Done { ok: bool, detail: String },
}

#[derive(Clone, Serialize)]
pub struct DeviceInfo {
  pub connected: bool,
  pub serial: Option<String>,
  pub vars: HashMap<String, String>,
}

/// Cooperative cancellation flags for the single in-flight flash /
/// force-fastboot operation. The GUI guarantees only one runs at a time.
#[derive(Default)]
struct CancelState {
  flash: Arc<AtomicBool>,
  force_fastboot: Arc<AtomicBool>,
}

// ── Helpers ───────────────────────────────────────────────────────────

fn send_progress(ch: &Channel<ProgressEvent>, event: ProgressEvent) {
  trace!(?event, "progress");
  let _ = ch.send(event);
}


// ── Commands ──────────────────────────────────────────────────────────

#[tracing::instrument(skip_all)]
#[tauri::command]
async fn get_device_info() -> Result<DeviceInfo, String> {
  let Ok(mut executor) = FlashExecutor::connect().await else {
    info!("no fastboot device found");
    return Ok(DeviceInfo { connected: false, serial: None, vars: HashMap::new() });
  };
  let vars = executor.get_all_vars().await.map_err(|e| {
    warn!(error = %e, "get_all_vars failed");
    e.to_string()
  })?;
  let serial = vars.get("serialno").cloned();
  let connected = true;
  info!(connected, serial = serial.as_deref().unwrap_or("?"), "device info retrieved");
  Ok(DeviceInfo { connected, serial, vars })
}

/// Poll the cancellation flag until it is set, so a long wait can be aborted
/// by `cancel_force_fastboot`.
async fn wait_for_cancel(flag: &AtomicBool) {
  while !flag.load(Ordering::Relaxed) {
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
  }
}

#[tracing::instrument(skip(app, on_event, cancel))]
#[tauri::command]
async fn force_fastboot(
  app: tauri::AppHandle,
  on_event: Channel<ProgressEvent>,
  cancel: State<'_, CancelState>,
) -> Result<(), String> {
  cancel.force_fastboot.store(false, Ordering::Relaxed);

  if pawflash_core::force_fastboot::fastboot::in_fastboot_mode().await {
    info!("already in fastboot mode");
    send_progress(&on_event, ProgressEvent::ForceFastbootStage { stage: "confirmed".into(), message: "Device already in fastboot mode.".into() });
    send_progress(&on_event, ProgressEvent::Done { ok: true, detail: "Already in fastboot mode".into() });
    let _ = app.emit("fastboot-devices", ());
    return Ok(());
  }

  send_progress(&on_event, ProgressEvent::ForceFastbootStage { stage: "waiting_preloader".into(), message: "Waiting for MediaTek preloader serial port...".into() });

  // Wait for the preloader, aborting early on cancel.
  let port = tokio::select! {
    result = pawflash_core::force_fastboot::serial::wait_for_preloader(false) => {
      match result {
        Ok(Some(port)) => port,
        Ok(None) => {
          warn!("no preloader device found");
          return Err("No preloader device found".into());
        }
        Err(e) => {
          warn!(error = %e, "wait_for_preloader failed");
          return Err(e.to_string());
        }
      }
    }
    _ = wait_for_cancel(&cancel.force_fastboot) => {
      info!("force fastboot cancelled while waiting for preloader");
      send_progress(&on_event, ProgressEvent::Cancelled { message: "Force fastboot cancelled".into() });
      return Ok(());
    }
  };

  let mut dev = pawflash_core::force_fastboot::serial::open_with_permission_recovery(&port)
    .map_err(|e| { warn!(%port, error = %e, "open_with_permission_recovery failed"); e.to_string() })?;

  info!(%port, "preloader found, sending FASTBOOT");
  send_progress(&on_event, ProgressEvent::ForceFastbootStage { stage: "sending".into(), message: format!("Found preloader on {port}, sending FASTBOOT...") });

  loop {
    if cancel.force_fastboot.load(Ordering::Relaxed) {
      info!("force fastboot cancelled during handshake");
      send_progress(&on_event, ProgressEvent::Cancelled { message: "Force fastboot cancelled".into() });
      return Ok(());
    }

    use tokio::io::AsyncWriteExt;
    match dev.write_all(b"FASTBOOT").await {
      Ok(()) => { let _ = dev.flush().await; }
      Err(_) => {
        debug!("FASTBOOT write failed, checking mode and reconnecting");
        drop(dev);
        if pawflash_core::force_fastboot::fastboot::in_fastboot_mode().await {
          debug!("device already in fastboot mode after reconnect");
          break;
        }
        let Some(new_port) =
          pawflash_core::force_fastboot::serial::wait_for_preloader(true).await.map_err(|e| e.to_string())?
        else {
          warn!("preloader disappeared during handshake");
          break;
        };
        debug!(port = %new_port, "reconnecting to preloader");
        dev = pawflash_core::force_fastboot::serial::open_with_permission_recovery(&new_port)
          .map_err(|e| e.to_string())?;
        continue;
      }
    }
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    if pawflash_core::force_fastboot::fastboot::in_fastboot_mode().await {
      debug!("fastboot mode confirmed");
      break;
    }
  }

  send_progress(&on_event, ProgressEvent::ForceFastbootStage { stage: "confirmed".into(), message: "Fastboot mode confirmed.".into() });
  info!("device now in fastboot mode");
  send_progress(&on_event, ProgressEvent::Done { ok: true, detail: "Device now in fastboot mode".into() });
  let _ = app.emit("fastboot-devices", ());
  Ok(())
}

#[tracing::instrument(skip(cancel))]
#[tauri::command]
async fn cancel_force_fastboot(cancel: State<'_, CancelState>) -> Result<(), String> {
  info!("cancel_force_fastboot requested");
  cancel.force_fastboot.store(true, Ordering::Relaxed);
  Ok(())
}

#[tracing::instrument(skip_all, fields(target))]
#[tauri::command]
async fn reboot_device(target: String) -> Result<(), String> {
  let mut executor = FlashExecutor::connect().await.map_err(|e| {
    warn!(error = %e, "connect failed");
    e.to_string()
  })?;
  let boot_target: BootTarget = target.parse().map_err(|e: String| e)?;
  info!(?boot_target, "rebooting");
  executor.reboot_to(boot_target).await.map_err(|e| {
    warn!(?boot_target, error = %e, "reboot failed");
    e.to_string()
  })
}

#[tracing::instrument(skip_all)]
#[tauri::command]
async fn lock_bootloader() -> Result<String, String> {
  let mut executor = FlashExecutor::connect().await.map_err(|e| {
    warn!(error = %e, "connect failed");
    e.to_string()
  })?;
  let resp = executor.flashing_lock().await.map_err(|e| {
    warn!(error = %e, "flashing lock failed");
    e.to_string()
  })?;
  info!(response = %resp, "bootloader locked");
  Ok(resp)
}

#[tracing::instrument(skip_all)]
#[tauri::command]
async fn unlock_bootloader() -> Result<String, String> {
  let mut executor = FlashExecutor::connect().await.map_err(|e| {
    warn!(error = %e, "connect failed");
    e.to_string()
  })?;
  let resp = executor.flashing_unlock().await.map_err(|e| {
    warn!(error = %e, "flashing unlock failed");
    e.to_string()
  })?;
  info!(response = %resp, "bootloader unlocked");
  Ok(resp)
}

#[tracing::instrument(skip_all, fields(slot))]
#[tauri::command]
async fn set_active_slot(slot: String) -> Result<String, String> {
  if slot != "a" && slot != "b" {
    warn!(%slot, "invalid slot");
    return Err("slot must be 'a' or 'b'".into());
  }
  let mut executor = FlashExecutor::connect().await.map_err(|e| {
    warn!(error = %e, "connect failed");
    e.to_string()
  })?;
  let resp = executor.set_active_slot(&slot).await.map_err(|e| {
    warn!(%slot, error = %e, "set_active_slot failed");
    e.to_string()
  })?;
  info!(%slot, response = %resp, "active slot set");
  Ok(resp)
}

#[tracing::instrument(skip_all, fields(name))]
#[tauri::command]
async fn get_var(name: String) -> Result<String, String> {
  let mut executor = FlashExecutor::connect().await.map_err(|e| {
    warn!(error = %e, "connect failed");
    e.to_string()
  })?;
  let value = executor.get_var(&name).await.map_err(|e| {
    warn!(%name, error = %e, "get_var failed");
    e.to_string()
  })?;
  info!(%name, %value, "variable retrieved");
  Ok(value)
}

#[tracing::instrument(skip(on_event))]
#[tauri::command]
async fn disable_vbmeta(on_event: Channel<ProgressEvent>) -> Result<(), String> {
  send_progress(&on_event, ProgressEvent::Phase { phase: "connecting".into(), message: "Connecting to device...".into() });
  let mut executor = FlashExecutor::connect().await.map_err(|e| {
    warn!(error = %e, "connect failed");
    e.to_string()
  })?;

  send_progress(&on_event, ProgressEvent::Phase { phase: "flashing".into(), message: "Flashing empty vbmeta...".into() });
  executor.flash_empty_vbmeta().await.map_err(|e| {
    warn!(error = %e, "flash_empty_vbmeta failed");
    e.to_string()
  })?;

  info!("vbmeta verification disabled");
  send_progress(&on_event, ProgressEvent::Done { ok: true, detail: "vbmeta verification disabled".into() });
  Ok(())
}

// ── Scatter commands ──────────────────────────────────────────────────

#[tracing::instrument(skip_all, fields(path))]
#[tauri::command]
async fn parse_scatter(path: String) -> Result<sp::ScatterFile, String> {
  let parsed = sp::parse_scatter(Path::new(&path)).map_err(|e| {
    warn!(%path, error = %e, "parse_scatter failed");
    e.to_string()
  })?;
  let count: usize = parsed.layouts.values().map(Vec::len).sum();
  info!(%path, partition_count = %count, "scatter parsed");
  Ok(parsed)
}

#[tracing::instrument(skip_all, fields(path))]
#[tauri::command]
async fn build_plan(path: String, options: sp::FlashPlanOptions) -> Result<sp::FlashPlan, String> {
  let parsed = sp::parse_scatter(Path::new(&path)).map_err(|e| {
    warn!(%path, error = %e, "parse_scatter for plan failed");
    e.to_string()
  })?;
  let plan = sp::build_flash_plan(&parsed, &options);
  info!(
    actions = %plan.actions.len(),
    skipped = %plan.skipped.len(),
    errors = %plan.errors.len(),
    "flash plan built"
  );
  Ok(plan)
}

#[tracing::instrument(skip(app, on_event, options, cancel), fields(path))]
#[tauri::command]
async fn execute_plan(
  app: tauri::AppHandle,
  path: String,
  options: sp::FlashPlanOptions,
  on_event: Channel<ProgressEvent>,
  cancel: State<'_, CancelState>,
) -> Result<pawflash_core::flash::results::FlashResult, String> {
  cancel.flash.store(false, Ordering::Relaxed);

  // Parse
  send_progress(&on_event, ProgressEvent::Phase { phase: "parsing".into(), message: "Parsing scatter file...".into() });
  let parsed = sp::parse_scatter(Path::new(&path)).map_err(|e| {
    warn!(%path, error = %e, "execute_plan: parse failed");
    e.to_string()
  })?;

  // Build plan
  send_progress(&on_event, ProgressEvent::Phase { phase: "planning".into(), message: "Building flash plan...".into() });
  let plan = sp::build_flash_plan(&parsed, &options);
  debug!(actions = %plan.actions.len(), skipped = %plan.skipped.len(), "plan built");

  if !plan.errors.is_empty() {
    for err in &plan.errors {
      warn!(%err, "plan error");
      send_progress(&on_event, ProgressEvent::Error { message: err.clone() });
    }
    return Err(format!("flash plan has {} error(s)", plan.errors.len()));
  }

  if plan.actions.is_empty() {
    warn!("flash plan has no actions");
    return Err("flash plan has no actions to execute".into());
  }

  // Connect
  send_progress(&on_event, ProgressEvent::Phase { phase: "connecting".into(), message: "Connecting to fastboot device...".into() });
  let mut executor = FlashExecutor::connect().await.map_err(|e| {
    warn!(error = %e, "execute_plan: connect failed");
    e.to_string()
  })?;

  // Execute with live byte-level progress streaming.
  let total = plan.actions.iter().filter(|a| a.action == "flash").count();
  info!(%total, "starting flash execution");
  send_progress(&on_event, ProgressEvent::Phase { phase: "flashing".into(), message: format!("Flashing {total} partitions...") });

  let mut on_transfer = |ev: FlashTransferEvent| {
    let _ = on_event.send(ProgressEvent::Flashing {
      partition: ev.partition,
      operation: ev.operation,
      bytes: ev.bytes,
      total: ev.total,
      overall_bytes: ev.overall_bytes,
      overall_total: ev.overall_total,
    });
  };

  let result = executor
    .execute_plan(
      &plan,
      FlashRunOptions {
        cancel: Some(&cancel.flash),
        on_transfer: Some(&mut on_transfer),
        ..Default::default()
      },
    )
    .await;

  let _ = app.emit("flash-complete", ());

  if result.cancelled {
    info!("flash plan cancelled by user");
    send_progress(&on_event, ProgressEvent::Cancelled { message: "Flash cancelled by user".into() });
    return Ok(result);
  }

  // Report outcomes
  for (i, outcome) in result.outcomes.iter().enumerate() {
    debug!(
      partition = %outcome.partition,
      success = %outcome.success,
      response = outcome.response.as_deref().unwrap_or(""),
      "flash outcome"
    );
    send_progress(&on_event, ProgressEvent::FlashProgress {
      partition: outcome.partition.clone(),
      percent: ((i + 1) as f64 / total as f64) * 100.0,
    });
    send_progress(&on_event, ProgressEvent::FlashComplete {
      partition: outcome.partition.clone(),
      success: outcome.success,
      response: outcome.response.clone(),
    });
    if let Some(ref err) = outcome.error.as_ref().filter(|_| !outcome.success) {
      warn!(partition = %outcome.partition, error = %err, "partition flash failed");
      send_progress(&on_event, ProgressEvent::Error { message: format!("{}: {err}", outcome.partition) });
    }
  }

  info!(
    succeeded = %result.succeeded,
    failed = %result.failed,
    total = %result.total,
    "flash execution complete"
  );
  send_progress(&on_event, ProgressEvent::Done {
    ok: result.failed == 0,
    detail: format!("{}/{} partitions flashed successfully", result.succeeded, result.total),
  });

  Ok(result)
}

#[tracing::instrument(skip(cancel))]
#[tauri::command]
async fn cancel_flash(cancel: State<'_, CancelState>) -> Result<(), String> {
  info!("cancel_flash requested");
  cancel.flash.store(true, Ordering::Relaxed);
  Ok(())
}

#[tracing::instrument(skip(app, on_event), fields(partition, image_path))]
#[tauri::command]
async fn flash_raw_image(
  app: tauri::AppHandle,
  partition: String,
  image_path: String,
  on_event: Channel<ProgressEvent>,
) -> Result<String, String> {
  send_progress(&on_event, ProgressEvent::Phase { phase: "connecting".into(), message: "Connecting to device...".into() });
  let mut executor = FlashExecutor::connect().await.map_err(|e| {
    warn!(error = %e, "connect failed");
    e.to_string()
  })?;

  let path = Path::new(&image_path);
  if !path.exists() {
    warn!(%image_path, "image not found");
    return Err(format!("image not found: {image_path}"));
  }

  send_progress(&on_event, ProgressEvent::Phase { phase: "flashing".into(), message: format!("Flashing {partition}...") });
  debug!(%partition, %image_path, "flashing raw image");
  let resp = executor.flash_raw_image(&partition, path).await.map_err(|e| {
    warn!(%partition, error = %e, "flash_raw_image failed");
    e.to_string()
  })?;

  let _ = app.emit("flash-complete", ());
  info!(%partition, response = %resp, "raw flash complete");
  send_progress(&on_event, ProgressEvent::FlashComplete { partition, success: true, response: Some(resp.clone()) });
  send_progress(&on_event, ProgressEvent::Done { ok: true, detail: "Raw flash complete".into() });

  Ok(resp)
}


// ── App entry ─────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  init_logging();
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .manage(CancelState::default())
    .invoke_handler(tauri::generate_handler![
      get_device_info,
      force_fastboot,
      cancel_force_fastboot,
      reboot_device,
      lock_bootloader,
      unlock_bootloader,
      set_active_slot,
      get_var,
      disable_vbmeta,
      parse_scatter,
      build_plan,
      execute_plan,
      cancel_flash,
      flash_raw_image,

    ])
    .run(tauri::generate_context!())
    .expect("error while running pawflash");
}
