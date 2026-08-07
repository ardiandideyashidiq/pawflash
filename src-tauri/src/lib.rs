//! Tauri v2 desktop app for pawflash — exposes core flashing operations as
//! IPC commands with progress reporting via `Channel<ProgressEvent>`.

mod sim;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use pawflash_core::flash::executor::BootTarget;
use pawflash_core::flash::progress::{FlashRunOptions, FlashTransferEvent};
use pawflash_core::flash::FlashExecutor;
use pawflash_core::scatter_parser as sp;
use pawflash_core::flash::simulate::simulated_vars;
use serde::Serialize;
use sim::AnyExecutor;
use tauri::ipc::Channel;
use tauri::State;
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
/// force-fastboot operation. The GUI guarantees only one runs at a time;
/// the `in_flight` flag enforces that at the backend so a duplicate invoke
/// (double-click, stale render) cannot start a second device-write operation.
#[derive(Default, Debug)]
struct CancelState {
  flash: Arc<AtomicBool>,
  force_fastboot: Arc<AtomicBool>,
  in_flight: Arc<AtomicBool>,
}

/// Acquire the single-operation guard, failing if another device-write
/// operation is already running.
fn acquire_guard(cancel: &CancelState) -> Result<(), String> {
  if cancel.in_flight.swap(true, Ordering::AcqRel) {
    return Err("another flash/device operation is already running".into());
  }
  Ok(())
}

/// RAII guard that releases `in_flight` on drop, covering every early-return
/// and `?` exit path of the command it guards.
struct OpGuard<'a> {
  cancel: &'a CancelState,
}

impl<'a> OpGuard<'a> {
  fn new(cancel: &'a CancelState) -> Result<Self, String> {
    acquire_guard(cancel)?;
    Ok(Self { cancel })
  }
}

impl Drop for OpGuard<'_> {
  fn drop(&mut self) {
    self.cancel.in_flight.store(false, Ordering::Release);
  }
}

/// In-memory cache of parsed scatter files, keyed by path and invalidated on
/// mtime/size change. A single GUI flash session parses the scatter through
/// three commands (validate → plan → execute); this collapses that to one
/// parse and one disk read + hash. Bounded to a small FIFO so long-lived GUI
/// sessions don't grow without limit.
#[derive(Default)]
struct ScatterCache {
  inner: Mutex<ScatterCacheInner>,
}

#[derive(Default)]
struct ScatterCacheInner {
  entries: HashMap<PathBuf, CachedScatter>,
  order: std::collections::VecDeque<PathBuf>,
}

const SCATTER_CACHE_MAX: usize = 8;

struct CachedScatter {
  mtime: std::time::SystemTime,
  size: u64,
  parsed: Arc<sp::ScatterFile>,
}

impl ScatterCache {
  fn get_or_parse(&self, path: &Path) -> Result<Arc<sp::ScatterFile>, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    let mtime = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
    let size = meta.len();

    let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(cached) = inner
      .entries
      .get(path)
      .filter(|c| c.mtime == mtime && c.size == size)
    {
      return Ok(cached.parsed.clone());
    }

    let parsed = Arc::new(sp::parse_scatter(path).map_err(|e| e.to_string())?);
    inner.entries.insert(
      path.to_path_buf(),
      CachedScatter {
        mtime,
        size,
        parsed: parsed.clone(),
      },
    );
    inner.order.push_back(path.to_path_buf());
    while inner.order.len() > SCATTER_CACHE_MAX {
      if let Some(oldest) = inner.order.pop_front() {
        inner.entries.remove(&oldest);
      }
    }
    Ok(parsed)
  }
}

// ── Helpers ───────────────────────────────────────────────────────────

fn send_progress(ch: &Channel<ProgressEvent>, event: ProgressEvent) {
  trace!(?event, "progress");
  let _ = ch.send(event);
}


// ── Commands ──────────────────────────────────────────────────────────

#[tracing::instrument(skip_all, fields(simulate))]
#[tauri::command]
async fn get_device_info(simulate: bool) -> Result<DeviceInfo, String> {
  if simulate {
    info!("simulated device info requested");
    let vars = simulated_vars();
    return Ok(DeviceInfo { connected: true, serial: Some("SIM000001".into()), vars });
  }

  match FlashExecutor::connect().await {
    Ok(mut executor) => {
      let vars = executor.get_all_vars().await.map_err(|e| {
        warn!(error = %e, "get_all_vars failed");
        e.to_string()
      })?;
      let serial = vars.get("serialno").cloned();
      let connected = true;
      info!(connected, serial = serial.as_deref().unwrap_or("?"), "device info retrieved");
      Ok(DeviceInfo { connected, serial, vars })
    }
    Err(pawflash_core::flash::FlashError::NoDevice) => {
      info!("no fastboot device found");
      Ok(DeviceInfo { connected: false, serial: None, vars: HashMap::new() })
    }
    Err(e) => {
      // Permissions, open failures, protocol errors — report them so the GUI
      // does not silently present "not connected".
      warn!(error = %e, "get_device_info: connect failed");
      Err(e.to_string())
    }
  }
}

/// Poll the cancellation flag until it is set, so a long wait can be aborted
/// by `cancel_force_fastboot`.
async fn wait_for_cancel(flag: &AtomicBool) {
  while !flag.load(Ordering::Relaxed) {
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
  }
}

#[tracing::instrument(skip(on_event, cancel), fields(simulate))]
#[tauri::command]
async fn force_fastboot(
  on_event: Channel<ProgressEvent>,
  cancel: State<'_, CancelState>,
  simulate: bool,
) -> Result<(), String> {
  cancel.force_fastboot.store(false, Ordering::Relaxed);
  let _guard = OpGuard::new(&cancel)?;

  if simulate {
    info!("simulated force fastboot");
    send_progress(&on_event, ProgressEvent::Warning { message: "SIMULATED MODE — no device will be touched".into() });
    return run_simulated_force_fastboot(&on_event, &cancel).await;
  }

  if pawflash_core::force_fastboot::fastboot::in_fastboot_mode().await {
    info!("already in fastboot mode");
    send_progress(&on_event, ProgressEvent::ForceFastbootStage { stage: "confirmed".into(), message: "Device already in fastboot mode.".into() });
    send_progress(&on_event, ProgressEvent::Done { ok: true, detail: "Already in fastboot mode".into() });
    return Ok(());
  }

  send_progress(&on_event, ProgressEvent::ForceFastbootStage { stage: "waiting_preloader".into(), message: "Waiting for MediaTek preloader serial port...".into() });

  // Wait for the preloader, aborting early on cancel. Passing `true` makes the
  // wait also detect a device that enters fastboot before a new serial port
  // appears, so the user is not forced to sit out the full 120s.
  let port = tokio::select! {
    result = pawflash_core::force_fastboot::serial::wait_for_preloader(true) => {
      match result {
        Ok(Some(port)) => port,
        Ok(None) => {
          info!("device entered fastboot while waiting for preloader");
          send_progress(&on_event, ProgressEvent::Done { ok: true, detail: "Device already in fastboot mode".into() });
          return Ok(());
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
  Ok(())
}

/// Simulated force-fastboot handshake: staged progress events with realistic
/// timing, abortable via the cancellation flag.
async fn run_simulated_force_fastboot(
  on_event: &Channel<ProgressEvent>,
  cancel: &CancelState,
) -> Result<(), String> {
  send_progress(
    on_event,
    ProgressEvent::ForceFastbootStage {
      stage: "waiting_preloader".into(),
      message: "Simulated: scanning for MediaTek preloader serial port...".into(),
    },
  );

  tokio::select! {
    () = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
    () = wait_for_cancel(&cancel.force_fastboot) => {
      info!("simulated force fastboot cancelled while waiting for preloader");
      send_progress(on_event, ProgressEvent::Cancelled { message: "Force fastboot cancelled".into() });
      return Ok(());
    }
  }

  send_progress(
    on_event,
    ProgressEvent::ForceFastbootStage {
      stage: "sending".into(),
      message: "Simulated: preloader found, sending FASTBOOT...".into(),
    },
  );
  tokio::time::sleep(std::time::Duration::from_secs(1)).await;

  send_progress(
    on_event,
    ProgressEvent::ForceFastbootStage {
      stage: "confirmed".into(),
      message: "Fastboot mode confirmed (simulated).".into(),
    },
  );
  info!("simulated force-fastboot handshake complete");
  send_progress(
    on_event,
    ProgressEvent::Done { ok: true, detail: "Device now in fastboot mode (simulated)".into() },
  );
  Ok(())
}

#[tracing::instrument(skip(cancel))]
#[tauri::command]
async fn cancel_force_fastboot(cancel: State<'_, CancelState>) -> Result<(), String> {
  info!("cancel_force_fastboot requested");
  cancel.force_fastboot.store(true, Ordering::Relaxed);
  Ok(())
}

#[tracing::instrument(skip_all, fields(target, simulate))]
#[tauri::command]
async fn reboot_device(target: String, simulate: bool) -> Result<(), String> {
  let boot_target: BootTarget = target.parse().map_err(|e: String| e)?;
  let mut executor = AnyExecutor::connect(simulate, None).await?;
  info!(?boot_target, %simulate, "rebooting");
  executor.reboot_to(boot_target).await.map_err(|e| {
    warn!(?boot_target, error = %e, "reboot failed");
    e
  })
}

#[tracing::instrument(skip_all, fields(simulate))]
#[tauri::command]
async fn lock_bootloader(simulate: bool) -> Result<String, String> {
  let mut executor = AnyExecutor::connect(simulate, None).await?;
  let resp = executor.flashing_lock().await.map_err(|e| {
    warn!(error = %e, "flashing lock failed");
    e
  })?;
  info!(response = %resp, %simulate, "bootloader locked");
  Ok(resp)
}

#[tracing::instrument(skip_all, fields(simulate))]
#[tauri::command]
async fn unlock_bootloader(simulate: bool) -> Result<String, String> {
  let mut executor = AnyExecutor::connect(simulate, None).await?;
  let resp = executor.flashing_unlock().await.map_err(|e| {
    warn!(error = %e, "flashing unlock failed");
    e
  })?;
  info!(response = %resp, %simulate, "bootloader unlocked");
  Ok(resp)
}

#[tracing::instrument(skip_all, fields(slot, simulate))]
#[tauri::command]
async fn set_active_slot(slot: String, simulate: bool) -> Result<String, String> {
  if slot != "a" && slot != "b" {
    warn!(%slot, "invalid slot");
    return Err("slot must be 'a' or 'b'".into());
  }
  let mut executor = AnyExecutor::connect(simulate, None).await?;
  let resp = executor.set_active_slot(&slot).await.map_err(|e| {
    warn!(%slot, error = %e, "set_active_slot failed");
    e
  })?;
  info!(%slot, response = %resp, %simulate, "active slot set");
  Ok(resp)
}

#[tracing::instrument(skip_all, fields(name, simulate))]
#[tauri::command]
async fn get_var(name: String, simulate: bool) -> Result<String, String> {
  let mut executor = AnyExecutor::connect(simulate, None).await?;
  let value = executor.get_var(&name).await.map_err(|e| {
    warn!(%name, error = %e, "get_var failed");
    e
  })?;
  info!(%name, %value, %simulate, "variable retrieved");
  Ok(value)
}

#[tracing::instrument(skip(on_event), fields(simulate))]
#[tauri::command]
async fn disable_vbmeta(on_event: Channel<ProgressEvent>, cancel: State<'_, CancelState>, simulate: bool) -> Result<(), String> {
  let _guard = OpGuard::new(&cancel)?;
  send_progress(&on_event, ProgressEvent::Phase { phase: "connecting".into(), message: "Connecting to device...".into() });
  if simulate {
    send_progress(&on_event, ProgressEvent::Warning { message: "SIMULATED MODE — no device will be touched".into() });
  }
  let mut executor = AnyExecutor::connect(simulate, None).await.map_err(|e| {
    warn!(error = %e, "connect failed");
    e
  })?;

  // vbmeta is only flashable from bootloader fastboot, not fastbootd.
  let is_userspace = executor.get_var("is-userspace").await.unwrap_or_default();
  if is_userspace == "yes" || is_userspace == "true" {
    let msg = "device is in fastbootd mode; vbmeta can only be flashed in bootloader mode. Reboot to bootloader first.";
    warn!(%msg);
    send_progress(&on_event, ProgressEvent::Error { message: msg.into() });
    return Err(msg.into());
  }

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

#[tracing::instrument(skip(cache), fields(path))]
#[tauri::command]
async fn parse_scatter(path: String, cache: State<'_, ScatterCache>) -> Result<sp::ScatterFile, String> {
  let parsed = cache.get_or_parse(Path::new(&path))?;
  let count: usize = parsed.layouts.values().map(Vec::len).sum();
  info!(%path, partition_count = %count, "scatter parsed");
  Ok((*parsed).clone())
}

#[tracing::instrument(skip(cache), fields(path))]
#[tauri::command]
async fn build_plan(
  path: String,
  options: sp::FlashPlanOptions,
  cache: State<'_, ScatterCache>,
) -> Result<sp::FlashPlan, String> {
  let parsed = cache.get_or_parse(Path::new(&path))?;
  let plan = sp::build_flash_plan(&parsed, &options);
  info!(
    actions = %plan.actions.len(),
    skipped = %plan.skipped.len(),
    errors = %plan.errors.len(),
    "flash plan built"
  );
  Ok(plan)
}

#[tracing::instrument(skip(on_event, options, cancel, cache), fields(path, simulate))]
#[tauri::command]
async fn execute_plan(
  path: String,
  options: sp::FlashPlanOptions,
  on_event: Channel<ProgressEvent>,
  cancel: State<'_, CancelState>,
  cache: State<'_, ScatterCache>,
  simulate: bool,
) -> Result<pawflash_core::flash::results::FlashResult, String> {
  cancel.flash.store(false, Ordering::Relaxed);
  let _guard = OpGuard::new(&cancel)?;

  // Parse
  send_progress(&on_event, ProgressEvent::Phase { phase: "parsing".into(), message: "Parsing scatter file...".into() });
  let parsed = cache.get_or_parse(Path::new(&path))?;

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
  if simulate {
    send_progress(&on_event, ProgressEvent::Warning { message: "SIMULATED MODE — no device will be touched".into() });
    send_progress(&on_event, ProgressEvent::Phase { phase: "connecting".into(), message: "Connecting to simulated device...".into() });
  } else {
    send_progress(&on_event, ProgressEvent::Phase { phase: "connecting".into(), message: "Connecting to fastboot device...".into() });
  }
  let mut executor = AnyExecutor::connect(simulate, Some(parsed.as_ref())).await.map_err(|e| {
    warn!(error = %e, "execute_plan: connect failed");
    e
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

  if result.cancelled {
    info!("flash plan cancelled by user");
    send_progress(&on_event, ProgressEvent::Cancelled { message: "Flash cancelled by user".into() });
    return Ok(result);
  }

  // Report outcomes
  let processed = result.outcomes.len().max(1);
  for (i, outcome) in result.outcomes.iter().enumerate() {
    debug!(
      partition = %outcome.partition,
      success = %outcome.success,
      response = outcome.response.as_deref().unwrap_or(""),
      "flash outcome"
    );
    send_progress(&on_event, ProgressEvent::FlashProgress {
      partition: outcome.partition.clone(),
      percent: ((i + 1) as f64 / processed as f64) * 100.0,
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

#[tracing::instrument(skip(on_event), fields(partition, image_path, simulate))]
#[tauri::command]
async fn flash_raw_image(
  partition: String,
  image_path: String,
  on_event: Channel<ProgressEvent>,
  cancel: State<'_, CancelState>,
  simulate: bool,
) -> Result<String, String> {
  let _guard = OpGuard::new(&cancel)?;
  send_progress(&on_event, ProgressEvent::Phase { phase: "connecting".into(), message: "Connecting to device...".into() });
  if simulate {
    send_progress(&on_event, ProgressEvent::Warning { message: "SIMULATED MODE — no device will be touched".into() });
  }
  let mut executor = AnyExecutor::connect(simulate, None).await.map_err(|e| {
    warn!(error = %e, "connect failed");
    e
  })?;

  let path = Path::new(&image_path);
  if !path.exists() {
    warn!(%image_path, "image not found");
    return Err(format!("image not found: {image_path}"));
  }

  // Resolve the target like the CLI: on an A/B device a bare partition name
  // means "the current slot", so flash `{partition}_{current}`. A partition
  // that already carries a slot suffix is used verbatim.
  let target = if partition.ends_with("_a") || partition.ends_with("_b") {
    partition.clone()
  } else if let Some(slot) = executor.device_vars().get("current-slot") {
    if slot == "a" || slot == "b" {
      let resolved = format!("{partition}_{slot}");
      info!(partition = %partition, target = %resolved, "resolved bare partition to current slot");
      send_progress(
        &on_event,
        ProgressEvent::DeviceAction {
          action: "resolve_target".into(),
          detail: format!("{partition} → {resolved}"),
        },
      );
      resolved
    } else {
      partition.clone()
    }
  } else {
    partition.clone()
  };

  send_progress(&on_event, ProgressEvent::Phase { phase: "flashing".into(), message: format!("Flashing {target}...") });
  debug!(%target, %image_path, "flashing raw image");
  if pawflash_core::scatter_parser::safety::requires_raw_flash_ack(&target) {
    let role = pawflash_core::scatter_parser::safety::role_for_name(&target);
    warn!(%target, %role, "refusing raw flash of safety-critical partition without confirmation");
    send_progress(&on_event, ProgressEvent::Error {
      message: format!("{target} is a {role} partition; raw-flashing it can brick or wipe the device."),
    });
    return Err(format!("{target} is a {role} partition; refusing without explicit confirmation"));
  }
  let resp = executor.flash_raw_image(&target, path).await.map_err(|e| {
    warn!(%target, error = %e, "flash_raw_image failed");
    e.to_string()
  })?;

  info!(%target, response = %resp, "raw flash complete");
  send_progress(&on_event, ProgressEvent::FlashComplete { partition: target, success: true, response: Some(resp.clone()) });
  send_progress(&on_event, ProgressEvent::Done { ok: true, detail: "Raw flash complete".into() });

  Ok(resp)
}

/// Role label for a partition name, surfaced in the ManualFlash UI so the
/// operator sees the risk before pressing the flash button.
#[tauri::command]
fn classify_partition(name: String) -> String {
  pawflash_core::scatter_parser::safety::role_for_name(&name)
}

// ── App entry ─────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  init_logging();
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .manage(CancelState::default())
    .manage(ScatterCache::default())
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
      classify_partition,

    ])
    .run(tauri::generate_context!())
    .expect("error while running pawflash");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_acquire_release_reacquire() {
        let cancel = CancelState::default();
        let guard = OpGuard::new(&cancel).expect("first acquire succeeds");
        drop(guard);
        let _guard = OpGuard::new(&cancel).expect("re-acquire after drop succeeds");
    }

    #[test]
    fn guard_refuses_concurrent_acquire() {
        let cancel = CancelState::default();
        let _guard = OpGuard::new(&cancel).expect("first acquire succeeds");
        let second = OpGuard::new(&cancel);
        assert!(second.is_err(), "second concurrent acquire must fail");
    }

    #[test]
    fn guard_drop_releases_for_next_acquire() {
        let cancel = CancelState::default();
        {
            let _guard = OpGuard::new(&cancel).expect("first acquire succeeds");
        }
        let _guard = OpGuard::new(&cancel).expect("acquire succeeds after drop");
    }
}
