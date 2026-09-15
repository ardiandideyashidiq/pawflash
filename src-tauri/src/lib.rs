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
use serde::{Deserialize, Serialize};
use sim::AnyExecutor;
use tauri::ipc::Channel;
use tauri::State;
use tracing::{debug, info, trace, warn};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, registry::Registry};

// ── Logging init ──────────────────────────────────────────────────────

fn init_logging() {
  let level = if std::env::var_os("PAWFLASH_DEBUG").is_some()
    || std::env::var("RUST_LOG").is_ok_and(|v| v.eq_ignore_ascii_case("debug") || v.eq_ignore_ascii_case("trace"))
  {
    LevelFilter::DEBUG
  } else {
    LevelFilter::INFO
  };

  let subscriber = Registry::default()
    .with(level)
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
  MtkPhase { phase: String, message: String },
  MtkProgress { bytes: u64, total: u64 },
  MtkDone { ok: bool, detail: String },
  PenumbraPhase { phase: String, message: String },
  PenumbraProgress { bytes: u64, total: u64 },
  PenumbraDone { ok: bool, detail: String },
  Done { ok: bool, detail: String },
}

#[derive(Clone, Serialize)]
pub struct DeviceInfo {
  pub connected: bool,
  pub serial: Option<String>,
  pub vars: HashMap<String, String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub hint: Option<String>,
}

/// Serializable error DTO for the Tauri boundary. Core `FlashError` variants
/// are mapped to a tagged kind so the GUI can render error-class-specific
/// guidance instead of a raw string; unknown errors fall back to `Other`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", content = "detail")]
pub enum AppError {
  NoDevice { message: String },
  Permission { message: String },
  Protocol { message: String },
  ActionFailed { partition: String, message: String },
  Cancelled { message: String },
  Timeout { message: String },
  Other { message: String },
}

impl From<String> for AppError {
  fn from(message: String) -> Self {
    Self::Other { message }
  }
}

impl From<&str> for AppError {
  fn from(message: &str) -> Self {
    Self::Other { message: message.to_string() }
  }
}

impl From<pawflash_core::flash::error::FlashError> for AppError {
  fn from(e: pawflash_core::flash::error::FlashError) -> Self {
    match e {
      pawflash_core::flash::error::FlashError::NoDevice => Self::NoDevice { message: e.to_string() },
      pawflash_core::flash::error::FlashError::Timeout { partition, step } => Self::Timeout {
        message: format!("flash transfer timed out: {partition}: {step}"),
      },
      pawflash_core::flash::error::FlashError::Cancelled => Self::Cancelled { message: "flash cancelled".into() },
      pawflash_core::flash::error::FlashError::ActionFailed { partition, reason } => {
        Self::ActionFailed { partition, message: reason }
      }
      other => Self::Other { message: other.to_string() },
    }
  }
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
  /// Token for the current operation's connect-wait phase. Each operation
  /// installs a fresh token at entry (a fired token is consumed); `cancel_flash`
  /// fires the current one so a pending `wait_for_device` aborts immediately.
  cancel_token: Mutex<Arc<tokio_util::sync::CancellationToken>>,
}

/// Acquire the single-operation guard, failing if another device-write
/// operation is already running.
fn acquire_guard(cancel: &CancelState) -> Result<(), String> {
  if cancel.in_flight.swap(true, Ordering::AcqRel) {
    return Err("another flash/device operation is already running".into());
  }
  Ok(())
}

/// Install a fresh cancellation token for the current operation and return a
/// clone for the caller's connect-wait phase. A fired token is consumed, so
/// each operation must start with an unfired one.
fn fresh_cancel_token(cancel: &CancelState) -> tokio_util::sync::CancellationToken {
  let token = tokio_util::sync::CancellationToken::new();
  *cancel.cancel_token.lock().unwrap_or_else(|p| p.into_inner()) = Arc::new(token.clone());
  token
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

static DEVICE_CHECK_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tracing::instrument(skip(cancel), fields(simulate))]
#[tauri::command]
async fn get_device_info(
  cancel: State<'_, CancelState>,
  simulate: bool,
) -> Result<DeviceInfo, AppError> {
  if simulate {
    info!("simulated device info requested");
    let vars = simulated_vars();
    return Ok(DeviceInfo { connected: true, serial: Some("SIM000001".into()), vars, hint: None });
  }

  if cancel.in_flight.load(Ordering::Relaxed) {
    debug!("device check skipped: operation in flight");
    return Ok(DeviceInfo {
      connected: false,
      serial: None,
      vars: HashMap::new(),
      hint: Some("Device operation in progress".into()),
    });
  }

  let _lock = DEVICE_CHECK_LOCK.lock().await;

  if cancel.in_flight.load(Ordering::Relaxed) {
    debug!("device check skipped: operation in flight");
    return Ok(DeviceInfo {
      connected: false,
      serial: None,
      vars: HashMap::new(),
      hint: Some("Device operation in progress".into()),
    });
  }

  let t0 = std::time::Instant::now();
  match FlashExecutor::connect().await {
    Ok(executor) => {
      let vars = executor.device_vars().clone();
      let serial = vars.get("serialno").cloned();
      let connected = true;
      let elapsed = t0.elapsed();
      info!(
        connected,
        serial = serial.as_deref().unwrap_or("?"),
        is_userspace = vars.get("is-userspace").map_or("no", |s| s.as_str()),
        ?elapsed,
        "device info retrieved"
      );
      Ok(DeviceInfo { connected, serial, vars, hint: None })
    }
    Err(e) => {
      let elapsed = t0.elapsed();
      match e {
        pawflash_core::flash::FlashError::NoDevice => {
          info!(?elapsed, "no fastboot device found");
          Ok(DeviceInfo { connected: false, serial: None, vars: HashMap::new(), hint: None })
        }
        pawflash_core::flash::FlashError::DeviceInAdb { .. }
        | pawflash_core::flash::FlashError::NoUsbInterface { .. }
        | pawflash_core::flash::FlashError::UnsupportedDriver { .. } => {
          // Detection diagnostics: report the reason as a hint so the GUI can
          // guide the user, but stay "not connected".
          warn!(?elapsed, error = %e, "get_device_info: no connectable fastboot device");
          Ok(DeviceInfo { connected: false, serial: None, vars: HashMap::new(), hint: Some(e.to_string()) })
        }
        other => {
          let msg = other.to_string();
          if msg.contains("busy") || msg.contains("16") {
            warn!(?elapsed, error = %other, "get_device_info: fastboot interface busy");
            Ok(DeviceInfo {
              connected: false,
              serial: None,
              vars: HashMap::new(),
              hint: Some("Fastboot interface busy".into()),
            })
          } else {
            // Permissions, open failures, protocol errors — report them so the GUI
            // does not silently present "not connected".
            warn!(?elapsed, error = %other, "get_device_info: connect failed");
            Err(AppError::from(other))
          }
        }
      }
    }
  }
}

#[tracing::instrument(skip(on_event, cancel), fields(simulate))]
#[tauri::command]
async fn force_fastboot(
  on_event: Channel<ProgressEvent>,
  cancel: State<'_, CancelState>,
  simulate: bool,
) -> Result<(), AppError> {
  cancel.force_fastboot.store(false, Ordering::Relaxed);
  let cancel_token = fresh_cancel_token(&cancel);
  let _guard = OpGuard::new(&cancel)?;

  if simulate {
    info!("simulated force fastboot");
    send_progress(&on_event, ProgressEvent::Warning { message: "SIMULATED MODE — no device will be touched".into() });
    return run_simulated_force_fastboot(&on_event, &cancel, &cancel_token).await;
  }

  if cancel_token.is_cancelled() || cancel.force_fastboot.load(Ordering::Relaxed) {
    send_progress(&on_event, ProgressEvent::Cancelled { message: "Force fastboot cancelled".into() });
    return Ok(());
  }

  if pawflash_core::force_fastboot::fastboot::in_fastboot_mode().await {
    info!("already in fastboot mode");
    pawflash_core::force_fastboot::fastboot::list_fastboot_devices().await;
    send_progress(&on_event, ProgressEvent::ForceFastbootStage { stage: "confirmed".into(), message: "Device already in fastboot mode.".into() });
    send_progress(&on_event, ProgressEvent::Done { ok: true, detail: "Already in fastboot mode".into() });
    return Ok(());
  }

  send_progress(&on_event, ProgressEvent::ForceFastbootStage { stage: "waiting_preloader".into(), message: "Waiting for MediaTek preloader serial port...".into() });

  let port = tokio::select! {
    biased;
    () = cancel_token.cancelled() => {
      info!("force fastboot cancelled while waiting for preloader");
      send_progress(&on_event, ProgressEvent::Cancelled { message: "Force fastboot cancelled".into() });
      return Ok(());
    }
    result = pawflash_core::force_fastboot::serial::wait_for_preloader_with_cancel(true, Some(&cancel.force_fastboot)) => {
      match result {
        Ok(Some(port)) => port,
        Ok(None) => {
          if cancel_token.is_cancelled() || cancel.force_fastboot.load(Ordering::Relaxed) {
            info!("force fastboot cancelled while waiting for preloader");
            send_progress(&on_event, ProgressEvent::Cancelled { message: "Force fastboot cancelled".into() });
            return Ok(());
          }
          info!("device entered fastboot while waiting for preloader");
          send_progress(&on_event, ProgressEvent::Done { ok: true, detail: "Device already in fastboot mode".into() });
          return Ok(());
        }
        Err(e) => {
          warn!(error = %e, "wait_for_preloader failed");
          return Err(e.to_string().into());
        }
      }
    }
  };

  let dev = tokio::select! {
    biased;
    () = cancel_token.cancelled() => {
      info!("force fastboot cancelled before opening serial port");
      send_progress(&on_event, ProgressEvent::Cancelled { message: "Force fastboot cancelled".into() });
      return Ok(());
    }
    dev_res = async { pawflash_core::force_fastboot::serial::open_with_permission_recovery(&port) } => {
      dev_res.map_err(|e| { warn!(%port, error = %e, "open_with_permission_recovery failed"); e.to_string() })?
    }
  };

  info!(%port, "preloader found, sending FASTBOOT");
  send_progress(&on_event, ProgressEvent::ForceFastbootStage { stage: "sending".into(), message: format!("Found preloader on {port}, sending FASTBOOT...") });

  let sends = pawflash_core::force_fastboot::handshake::handshake(
    dev,
    &port,
    Some(&cancel.force_fastboot),
    |event| match event {
      pawflash_core::force_fastboot::handshake::HandshakeEvent::Write { count } => {
        debug!(sends = count, "FASTBOOT write");
      }
      pawflash_core::force_fastboot::handshake::HandshakeEvent::PortLost { port } => {
        warn!(%port, "preloader port lost, waiting for reconnect");
      }
      pawflash_core::force_fastboot::handshake::HandshakeEvent::PortReconnected { port } => {
        debug!(port = %port, "reconnected to preloader");
      }
    },
  )
  .await
  .map_err(|e| {
    warn!(error = %e, "force-fastboot handshake failed");
    e.to_string()
  })?;

  if cancel.force_fastboot.load(Ordering::Relaxed) || cancel_token.is_cancelled() {
    info!(sends, "force fastboot cancelled during handshake");
    send_progress(&on_event, ProgressEvent::Cancelled { message: "Force fastboot cancelled".into() });
    return Ok(());
  }

  send_progress(&on_event, ProgressEvent::ForceFastbootStage { stage: "confirmed".into(), message: "Fastboot mode confirmed.".into() });
  info!(sends, "device now in fastboot mode");
  let hint = pawflash_core::platform::CURRENT.post_handshake_hint();
  if !hint.is_empty() && !pawflash_core::force_fastboot::fastboot::in_fastboot_mode().await {
    pawflash_core::force_fastboot::fastboot::log_fastboot_diagnostics().await;
    send_progress(&on_event, ProgressEvent::Warning {
      message: hint.into(),
    });
  } else {
    pawflash_core::force_fastboot::fastboot::list_fastboot_devices().await;
  }
  send_progress(&on_event, ProgressEvent::Done { ok: true, detail: "Device now in fastboot mode".into() });
  Ok(())
}

/// Simulated force-fastboot handshake: staged progress events with realistic
/// timing, abortable via the cancellation token.
async fn run_simulated_force_fastboot(
  on_event: &Channel<ProgressEvent>,
  cancel: &CancelState,
  cancel_token: &tokio_util::sync::CancellationToken,
) -> Result<(), AppError> {
  send_progress(
    on_event,
    ProgressEvent::ForceFastbootStage {
      stage: "waiting_preloader".into(),
      message: "Simulated: scanning for MediaTek preloader serial port...".into(),
    },
  );

  tokio::select! {
    biased;
    () = cancel_token.cancelled() => {
      info!("simulated force fastboot cancelled while waiting for preloader");
      send_progress(on_event, ProgressEvent::Cancelled { message: "Force fastboot cancelled".into() });
      return Ok(());
    }
    () = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
  }

  if cancel.force_fastboot.load(Ordering::Relaxed) || cancel_token.is_cancelled() {
    send_progress(on_event, ProgressEvent::Cancelled { message: "Force fastboot cancelled".into() });
    return Ok(());
  }

  send_progress(
    on_event,
    ProgressEvent::ForceFastbootStage {
      stage: "sending".into(),
      message: "Simulated: preloader found, sending FASTBOOT...".into(),
    },
  );

  tokio::select! {
    biased;
    () = cancel_token.cancelled() => {
      info!("simulated force fastboot cancelled while sending FASTBOOT");
      send_progress(on_event, ProgressEvent::Cancelled { message: "Force fastboot cancelled".into() });
      return Ok(());
    }
    () = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
  }

  if cancel.force_fastboot.load(Ordering::Relaxed) || cancel_token.is_cancelled() {
    send_progress(on_event, ProgressEvent::Cancelled { message: "Force fastboot cancelled".into() });
    return Ok(());
  }

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
async fn cancel_force_fastboot(cancel: State<'_, CancelState>) -> Result<(), AppError> {
  info!("cancel_force_fastboot requested");
  cancel.force_fastboot.store(true, Ordering::Relaxed);
  let token = cancel.cancel_token.lock().unwrap_or_else(|p| p.into_inner()).clone();
  token.cancel();
  Ok(())
}

#[tracing::instrument(skip_all, fields(target, simulate))]
#[tauri::command]
async fn reboot_device(target: String, simulate: bool) -> Result<(), AppError> {
  if target == "shutdown" {
    return shutdown_device(simulate).await;
  }
  let _lock = DEVICE_CHECK_LOCK.lock().await;
  let t0 = std::time::Instant::now();

  if target.starts_with("mtk:") {
    let mode_str = target.trim_start_matches("mtk:");
    let boot = match mode_str {
      "normal" | "system" => pawflash_core::penumbra::PenumbraBootMode::Normal,
      "homescreen" => pawflash_core::penumbra::PenumbraBootMode::HomeScreen,
      "fastboot" | "bootloader" => pawflash_core::penumbra::PenumbraBootMode::Fastboot,
      "meta" => pawflash_core::penumbra::PenumbraBootMode::Meta,
      "test" => pawflash_core::penumbra::PenumbraBootMode::Test,
      other => return Err(AppError::Other { message: format!("invalid MTK boot mode '{other}'") }),
    };
    let da = gui_da_bytes(simulate)?;
    info!(?boot, %simulate, "rebooting via penumbra");
    return tokio::task::spawn_blocking(move || {
      pawflash_core::penumbra::reboot(&da, boot, simulate, &mut |_| {})
    })
    .await
    .map_err(|e| AppError::Other { message: e.to_string() })?
    .map_err(|e| AppError::Other { message: penumbra_err_string(&e) });
  }

  let boot_target: BootTarget = target.parse().map_err(|e: String| e)?;
  let mut executor = match AnyExecutor::connect(simulate, None).await {
    Ok(exec) => exec,
    Err(e) => {
      if let Some(port_mode) = pawflash_core::penumbra::detect_mtk_port() {
        info!(?port_mode, "fastboot not found, routing reboot through detected MTK port");
        let da = gui_da_bytes(simulate)?;
        let boot = match boot_target {
          BootTarget::Bootloader | BootTarget::Fastboot => pawflash_core::penumbra::PenumbraBootMode::Fastboot,
          BootTarget::Recovery | BootTarget::System => pawflash_core::penumbra::PenumbraBootMode::Normal,
        };
        return tokio::task::spawn_blocking(move || {
          pawflash_core::penumbra::reboot(&da, boot, simulate, &mut |_| {})
        })
        .await
        .map_err(|err| AppError::Other { message: err.to_string() })?
        .map_err(|err| AppError::Other { message: penumbra_err_string(&err) });
      }
      return Err(AppError::from(e));
    }
  };
  let connect_duration = t0.elapsed();
  info!(?boot_target, %simulate, ?connect_duration, "rebooting");
  let t_reboot = std::time::Instant::now();
  executor.reboot_to(boot_target).await.map_err(|e| {
    let reboot_duration = t_reboot.elapsed();
    warn!(?boot_target, ?reboot_duration, error = %e, "reboot failed");
    AppError::from(e)
  })?;
  let reboot_duration = t_reboot.elapsed();
  let total = t0.elapsed();
  info!(?boot_target, ?connect_duration, ?reboot_duration, ?total, "reboot command succeeded");
  Ok(())
}

#[tracing::instrument(skip_all, fields(simulate))]
#[tauri::command]
async fn shutdown_device(simulate: bool) -> Result<(), AppError> {
  let _lock = DEVICE_CHECK_LOCK.lock().await;
  let da = gui_da_bytes(simulate)?;
  if simulate {
    info!("simulated shutdown succeeded");
    return Ok(());
  }
  tokio::task::spawn_blocking(move || {
    pawflash_core::penumbra::shutdown(&da, false, &mut |_| {})
  })
  .await
  .map_err(|e| AppError::Other { message: e.to_string() })?
  .map_err(|e| AppError::Other { message: penumbra_err_string(&e) })
}

#[tracing::instrument(skip_all, fields(simulate))]
#[tauri::command]
async fn lock_bootloader(simulate: bool) -> Result<String, AppError> {
  let _lock = DEVICE_CHECK_LOCK.lock().await;
  let t0 = std::time::Instant::now();
  let mut executor = AnyExecutor::connect(simulate, None).await?;
  let connect_duration = t0.elapsed();
  let resp = executor.flashing_lock().await.map_err(|e| {
    warn!(error = %e, "flashing lock failed");
    e
  })?;
  let total = t0.elapsed();
  info!(response = %resp, %simulate, ?connect_duration, ?total, "bootloader locked");
  Ok(resp)
}

#[tracing::instrument(skip_all, fields(simulate))]
#[tauri::command]
async fn unlock_bootloader(simulate: bool) -> Result<String, AppError> {
  let _lock = DEVICE_CHECK_LOCK.lock().await;
  let t0 = std::time::Instant::now();
  let mut executor = AnyExecutor::connect(simulate, None).await?;
  let connect_duration = t0.elapsed();
  let resp = executor.flashing_unlock().await.map_err(|e| {
    warn!(error = %e, "flashing unlock failed");
    e
  })?;
  let total = t0.elapsed();
  info!(response = %resp, %simulate, ?connect_duration, ?total, "bootloader unlocked");
  Ok(resp)
}

#[tracing::instrument(skip_all, fields(slot, simulate))]
#[tauri::command]
async fn set_active_slot(slot: String, simulate: bool) -> Result<String, AppError> {
  if slot != "a" && slot != "b" {
    warn!(%slot, "invalid slot");
    return Err("slot must be 'a' or 'b'".into());
  }
  let _lock = DEVICE_CHECK_LOCK.lock().await;
  let t0 = std::time::Instant::now();
  let mut executor = AnyExecutor::connect(simulate, None).await?;
  let connect_duration = t0.elapsed();
  let resp = executor.set_active_slot(&slot).await.map_err(|e| {
    warn!(%slot, error = %e, "set_active_slot failed");
    e
  })?;
  let total = t0.elapsed();
  info!(%slot, response = %resp, %simulate, ?connect_duration, ?total, "active slot set");
  Ok(resp)
}

#[tracing::instrument(skip_all, fields(name, simulate))]
#[tauri::command]
async fn get_var(name: String, simulate: bool) -> Result<String, AppError> {
  let _lock = DEVICE_CHECK_LOCK.lock().await;
  let t0 = std::time::Instant::now();
  let mut executor = AnyExecutor::connect(simulate, None).await?;
  let connect_duration = t0.elapsed();
  let value = executor.get_var(&name).await.map_err(|e| {
    warn!(%name, error = %e, "get_var failed");
    e
  })?;
  let total = t0.elapsed();
  info!(%name, %value, %simulate, ?connect_duration, ?total, "variable retrieved");
  Ok(value)
}

#[tracing::instrument(skip_all, fields(simulate))]
#[tauri::command]
async fn get_all_vars(simulate: bool) -> Result<HashMap<String, String>, AppError> {
  let _lock = DEVICE_CHECK_LOCK.lock().await;
  let t0 = std::time::Instant::now();
  let mut executor = AnyExecutor::connect(simulate, None).await?;
  let connect_duration = t0.elapsed();
  let vars = executor.get_all_vars().await.map_err(|e| {
    warn!(error = %e, "get_all_vars failed");
    e
  })?;
  let total = t0.elapsed();
  info!(count = vars.len(), %simulate, ?connect_duration, ?total, "all variables retrieved");
  Ok(vars)
}

#[tracing::instrument(skip(on_event), fields(simulate))]
#[tauri::command]
async fn disable_vbmeta(on_event: Channel<ProgressEvent>, cancel: State<'_, CancelState>, simulate: bool) -> Result<(), AppError> {
  let _guard = OpGuard::new(&cancel)?;
  send_progress(&on_event, ProgressEvent::Phase { phase: "connecting".into(), message: "Waiting for fastboot device...".into() });
  if simulate {
    send_progress(&on_event, ProgressEvent::Warning { message: "SIMULATED MODE — no device will be touched".into() });
  }
  let wait_token = fresh_cancel_token(&cancel);
  let mut executor = AnyExecutor::connect_wait(
    simulate,
    None,
    std::time::Duration::from_secs(60),
    wait_token,
  )
  .await
  .map_err(|e| {
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

// ── MTK DA commands ─────────────────────────────────────────────────

/// Status payload for the mtk bridge, returned by `mtk_status`.
#[derive(Debug, Clone, Serialize)]
pub struct MtkStatusPayload {
  pub version: Option<String>,
  pub path: Option<String>,
  pub installed: bool,
  pub device_visible: bool,
  pub platform: String,
}

/// Map a core `MtkError` to a GUI-friendly string.
fn mtk_err_string(e: &pawflash_core::mtk::MtkError) -> String {
  match e {
    pawflash_core::mtk::MtkError::DeviceBusy => {
      "another pawflash process is using the device".to_string()
    }
    pawflash_core::mtk::MtkError::HashMismatch { .. } => {
      "bridge download failed verification; try Download again".to_string()
    }
    pawflash_core::mtk::MtkError::MissingAsset { platform } => {
      format!("no bridge asset for platform {platform}")
    }
    other => other.to_string(),
  }
}

#[tracing::instrument(skip_all)]
#[tauri::command]
async fn mtk_status(simulate: bool) -> Result<MtkStatusPayload, AppError> {
  let platform = match pawflash_core::mtk::current_platform() {
    Ok(p) => p,
    Err(e) => return Err(AppError::Other { message: e.to_string() }),
  };
  let device_visible = if simulate {
    false
  } else {
    pawflash_core::udev::device_visible().await
  };
  let version = if simulate { None } else { pawflash_core::mtk::current_version() };
  let installed = version.is_some();
  let path = version.as_ref().map(|_| {
    let exe = pawflash_core::platform::CURRENT.bridge_binary_name();
    pawflash_core::mtk::install_root().join("bridge").join(exe).display().to_string()
  });
  Ok(MtkStatusPayload {
    version,
    path,
    installed,
    device_visible,
    platform,
  })
}

#[tracing::instrument(skip_all, fields(simulate))]
#[tauri::command]
async fn mtk_download(on_event: Channel<ProgressEvent>, simulate: bool) -> Result<(), AppError> {
  send_progress(&on_event, ProgressEvent::MtkPhase { phase: "manifest".into(), message: "Fetching bridge manifest...".into() });

  if simulate {
    // Simulated download: no network. Stream a realistic byte count so the UI
    // exercises the same progress path as a real download.
    send_progress(&on_event, ProgressEvent::MtkPhase { phase: "download".into(), message: "Downloading (simulated)...".into() });
    const SIM_TOTAL: u64 = 56 * 1024 * 1024;
    let channel = on_event.clone();
    tokio::task::spawn_blocking(move || {
      let mut done = 0u64;
      while done < SIM_TOTAL {
        done = (done + 1024 * 1024).min(SIM_TOTAL);
        let _ = channel.send(ProgressEvent::MtkProgress { bytes: done, total: SIM_TOTAL });
        std::thread::sleep(std::time::Duration::from_millis(8));
      }
    })
    .await
    .map_err(|e| AppError::Other { message: e.to_string() })?;
    send_progress(&on_event, ProgressEvent::MtkDone { ok: true, detail: "installed (simulated)".into() });
    return Ok(());
  }

  let manifest = pawflash_core::mtk::fetch_manifest().map_err(|e| AppError::Other { message: mtk_err_string(&e) })?;
  send_progress(&on_event, ProgressEvent::MtkPhase { phase: "download".into(), message: format!("Downloading {}...", manifest.version) });
  let channel = on_event.clone();
  let bin = tokio::task::spawn_blocking(move || {
    let mut last_sent = 0u64;
    let mut on_progress = |done: u64, total: u64| {
      // Throttle to one event per MiB so a large download doesn't spam the
      // channel; always emit the final tick.
      if done - last_sent >= 1024 * 1024 || done == total {
        last_sent = done;
        let _ = channel.send(ProgressEvent::MtkProgress { bytes: done, total });
      }
    };
    pawflash_core::mtk::ensure_installed(&manifest, Some(&mut on_progress))
  })
  .await
  .map_err(|e| AppError::Other { message: e.to_string() })?
  .map_err(|e| AppError::Other { message: mtk_err_string(&e) })?;
  send_progress(&on_event, ProgressEvent::MtkDone { ok: true, detail: format!("installed at {}", bin.display()) });
  Ok(())
}

#[tracing::instrument(skip_all)]
#[tauri::command]
async fn mtk_remove(on_event: Channel<ProgressEvent>) -> Result<(), AppError> {
  let root = pawflash_core::mtk::install_root();
  if !root.exists() {
    send_progress(&on_event, ProgressEvent::MtkDone { ok: true, detail: "mtk bridge not installed".into() });
    return Ok(());
  }
  std::fs::remove_dir_all(&root).map_err(|e| AppError::Other { message: e.to_string() })?;
  send_progress(&on_event, ProgressEvent::MtkDone { ok: true, detail: "mtk bridge removed".into() });
  Ok(())
}

#[tracing::instrument(skip_all)]
#[tauri::command]
async fn mtk_doctor(on_event: Channel<ProgressEvent>, simulate: bool) -> Result<(), AppError> {
  match pawflash_core::mtk::current_platform() {
    Ok(p) => send_progress(&on_event, ProgressEvent::MtkPhase { phase: "platform".into(), message: format!("platform: {p}") }),
    Err(e) => send_progress(&on_event, ProgressEvent::Error { message: e.to_string() }),
  }
  match pawflash_core::mtk::current_version() {
    Some(v) => send_progress(&on_event, ProgressEvent::MtkPhase { phase: "bridge".into(), message: format!("bridge installed ({v})") }),
    None => send_progress(&on_event, ProgressEvent::MtkPhase { phase: "bridge".into(), message: "bridge not installed".into() }),
  }
  if pawflash_core::platform::CURRENT.install_udev_rules() {
    send_progress(&on_event, ProgressEvent::MtkPhase { phase: "udev".into(), message: "udev rules installed".into() });
  } else {
    send_progress(&on_event, ProgressEvent::Error { message: "udev rules not installed".into() });
  }
  if let Err(e) = pawflash_core::platform::CURRENT.ensure_driver() {
    send_progress(&on_event, ProgressEvent::Error { message: e });
  }
  if !simulate {
    let visible = pawflash_core::udev::device_visible().await;
    send_progress(&on_event, ProgressEvent::MtkPhase { phase: "device".into(), message: if visible { "DA-capable device visible".into() } else { "no DA-capable device visible".into() } });
  }
  send_progress(&on_event, ProgressEvent::MtkDone { ok: true, detail: "doctor complete".into() });
  Ok(())
}

/// Run a blocking core mtk op, translating events into `ProgressEvent`s.
async fn run_mtk_op<F, T>(
  on_event: &Channel<ProgressEvent>,
  _simulate: bool,
  op: F,
) -> Result<T, AppError>
where
  F: FnOnce(&mut dyn FnMut(&pawflash_core::mtk::MtkEvent)) -> Result<T, pawflash_core::mtk::MtkError>
    + Send
    + 'static,
  T: Send + 'static,
{
  let channel = on_event.clone();
  tokio::task::spawn_blocking(move || {
    let mut emit = |ev: &pawflash_core::mtk::MtkEvent| {
      let _ = channel.send(match ev {
        pawflash_core::mtk::MtkEvent::Phase { phase, message } => {
          ProgressEvent::MtkPhase { phase: phase.clone(), message: message.clone() }
        }
        pawflash_core::mtk::MtkEvent::Start { total, partition } => {
          ProgressEvent::MtkPhase { phase: "start".into(), message: format!("{partition}: {total} bytes") }
        }
        pawflash_core::mtk::MtkEvent::Progress { bytes } => {
          ProgressEvent::MtkProgress { bytes: *bytes, total: 0 }
        }
        pawflash_core::mtk::MtkEvent::Log { level, message } => {
          ProgressEvent::MtkPhase { phase: "log".into(), message: format!("[{level}] {message}") }
        }
        pawflash_core::mtk::MtkEvent::Result { ok, detail, .. } => {
          ProgressEvent::MtkDone { ok: *ok, detail: detail.clone().unwrap_or_default() }
        }
        pawflash_core::mtk::MtkEvent::Error { message } => {
          ProgressEvent::Error { message: message.clone() }
        }
      });
    };
    op(&mut emit)
  })
  .await
  .map_err(|e| AppError::Other { message: e.to_string() })?
  .map_err(|e| AppError::Other { message: mtk_err_string(&e) })
}

/// Resolve the manifest for a GUI mtk op (dummy when simulating).
fn gui_manifest(simulate: bool) -> Result<pawflash_core::mtk::Manifest, AppError> {
  if simulate {
    return Ok(pawflash_core::mtk::Manifest {
      version: "simulated".into(),
      commit: String::new(),
      platforms: HashMap::new(),
    });
  }
  pawflash_core::mtk::fetch_manifest().map_err(|e| AppError::Other { message: mtk_err_string(&e) })
}

#[tracing::instrument(skip_all, fields(partition, simulate))]
#[tauri::command]
async fn mtk_read(
  partition: String,
  file: String,
  parttype: String,
  on_event: Channel<ProgressEvent>,
  simulate: bool,
) -> Result<u64, AppError> {
  let parttype = parttype_from_str(&parttype)?;
  let manifest = gui_manifest(simulate)?;
  send_progress(&on_event, ProgressEvent::MtkPhase { phase: "read".into(), message: format!("Reading {partition} → {file}") });
  run_mtk_op(&on_event, simulate, move |emit| {
    pawflash_core::mtk::read_partition(&manifest, &partition, Path::new(&file), parttype, simulate, emit)
  })
  .await
}

#[tracing::instrument(skip_all, fields(partition, simulate))]
#[tauri::command]
async fn mtk_write(
  partition: String,
  file: String,
  parttype: String,
  on_event: Channel<ProgressEvent>,
  simulate: bool,
) -> Result<u64, AppError> {
  let parttype = parttype_from_str(&parttype)?;
  let manifest = gui_manifest(simulate)?;
  if !simulate && !Path::new(&file).exists() {
    return Err(AppError::Other { message: format!("file not found: {file}") });
  }
  send_progress(&on_event, ProgressEvent::MtkPhase { phase: "write".into(), message: format!("Writing {file} → {partition}") });
  run_mtk_op(&on_event, simulate, move |emit| {
    pawflash_core::mtk::write_partition(&manifest, &partition, Path::new(&file), parttype, simulate, emit)
  })
  .await
}

#[tracing::instrument(skip_all, fields(partition, simulate))]
#[tauri::command]
async fn mtk_erase(
  partition: String,
  parttype: String,
  on_event: Channel<ProgressEvent>,
  simulate: bool,
) -> Result<(), AppError> {
  let parttype = parttype_from_str(&parttype)?;
  let manifest = gui_manifest(simulate)?;
  send_progress(&on_event, ProgressEvent::MtkPhase { phase: "erase".into(), message: format!("Erasing {partition}") });
  run_mtk_op(&on_event, simulate, move |emit| {
    pawflash_core::mtk::erase_partition(&manifest, &partition, parttype, simulate, emit)
  })
  .await
}

fn parttype_from_str(s: &str) -> Result<pawflash_core::mtk::PartType, AppError> {
  match s {
    "user" => Ok(pawflash_core::mtk::PartType::User),
    "boot1" => Ok(pawflash_core::mtk::PartType::Boot1),
    "boot2" => Ok(pawflash_core::mtk::PartType::Boot2),
    "rpmb" => Ok(pawflash_core::mtk::PartType::Rpmb),
    other => Err(AppError::Other { message: format!("invalid parttype '{other}'") }),
  }
}

// ── Penumbra DA commands ─────────────────────────────────────────────

/// Status payload for the penumbra DA integration.
#[derive(Debug, Clone, Serialize)]
pub struct PenumbraStatusPayload {
  pub da_version: Option<String>,
  pub da_path: Option<String>,
  pub da_installed: bool,
  pub device_visible: bool,
  pub platform: String,
  pub auth_path: Option<String>,
  pub is_custom: bool,
}

/// Map a core `PenumbraError` to a GUI-friendly string.
fn penumbra_err_string(e: &pawflash_core::penumbra::PenumbraError) -> String {
  match e {
    pawflash_core::penumbra::PenumbraError::DeviceBusy => {
      "another pawflash process is using the device".to_string()
    }
    pawflash_core::penumbra::PenumbraError::HashMismatch { .. } => {
      "DA download failed verification; try `da download` again".to_string()
    }
    pawflash_core::penumbra::PenumbraError::NoDaSelected => {
      "no DA selected — run `da download` first".to_string()
    }
    other => other.to_string(),
  }
}

/// Resolve DA bytes for a GUI op (`--da` style override or persisted selection).
fn gui_da_bytes(simulate: bool) -> Result<Vec<u8>, AppError> {
  if simulate {
    return Ok(Vec::new());
  }
  let sel = pawflash_core::penumbra::load_selection()
    .ok_or_else(|| AppError::Other { message: "no DA selected".into() })?;
  if !sel.sha256.is_empty() {
    pawflash_core::penumbra::verify_da(Path::new(&sel.path), &sel.sha256)
      .map_err(|e| AppError::Other { message: penumbra_err_string(&e) })?;
  }
  std::fs::read(&sel.path).map_err(|e| AppError::Other { message: e.to_string() })
}

/// Resolve optional auth bytes for a GUI op.
fn gui_auth_bytes(simulate: bool) -> Result<Option<Vec<u8>>, AppError> {
  if simulate {
    return Ok(None);
  }
  if let Some(sel) = pawflash_core::penumbra::load_selection()
    && let Some(auth_path) = sel.auth_path
    && !auth_path.trim().is_empty()
    && Path::new(&auth_path).is_file()
  {
    let bytes = std::fs::read(&auth_path).map_err(|e| AppError::Other { message: e.to_string() })?;
    return Ok(Some(bytes));
  }
  Ok(None)
}

#[tracing::instrument(skip_all)]
#[tauri::command]
async fn penumbra_status(simulate: bool) -> Result<PenumbraStatusPayload, AppError> {
  let platform = match pawflash_core::mtk::current_platform() {
    Ok(p) => p,
    Err(e) => return Err(AppError::Other { message: e.to_string() }),
  };
  let device_visible = if simulate {
    false
  } else {
    pawflash_core::udev::device_visible().await
  };
  let selection = if simulate { None } else { pawflash_core::penumbra::load_selection() };
  let da_version = selection.as_ref().map(|s| format!("{} ({})", s.brand, s.chipset));
  let da_installed = da_version.is_some();
  let da_path = selection.as_ref().map(|s| s.path.clone());
  let auth_path = selection.as_ref().and_then(|s| s.auth_path.clone());
  let is_custom = selection.as_ref().is_some_and(|s| s.is_custom);
  Ok(PenumbraStatusPayload {
    da_version,
    da_path,
    da_installed,
    device_visible,
    platform,
    auth_path,
    is_custom,
  })
}

#[tracing::instrument(skip_all, fields(simulate))]
#[tauri::command]
async fn penumbra_list_devices(
  simulate: bool,
) -> Result<Vec<pawflash_core::penumbra::DAEntry>, AppError> {
  if simulate {
    return Ok(vec![
      pawflash_core::penumbra::DAEntry {
        id: Some("infinix-mt6789".into()),
        brand: "infinix".into(),
        chipset: "mt6789".into(),
        devices: vec!["Infinix NOTE 12".into(), "Infinix Zero 20".into()],
        da: Some(pawflash_core::penumbra::manifest::FileBlob {
          url: "https://example.com/infinix-mt6789.bin".into(),
          sha256: "3c7de4ee52b47f1d4c5122868b52dfa06c18e5ef940f4c8a04c46365a696bbdd".into(),
          filename: Some("infinix-mt6789.bin".into()),
          size_bytes: Some(184320),
        }),
        auth: None,
        url: "https://example.com/infinix-mt6789.bin".into(),
        sha256: "3c7de4ee52b47f1d4c5122868b52dfa06c18e5ef940f4c8a04c46365a696bbdd".into(),
        auth_url: None,
        auth_sha256: None,
        verified: Some(true),
        notes: None,
      },
      pawflash_core::penumbra::DAEntry {
        id: Some("xiaomi-mt6877-combo".into()),
        brand: "xiaomi".into(),
        chipset: "mt6877".into(),
        devices: vec!["Redmi Note 12 Pro 5G".into(), "Redmi Note 12 Pro+ 5G".into()],
        da: Some(pawflash_core::penumbra::manifest::FileBlob {
          url: "https://example.com/xiaomi-mt6877.bin".into(),
          sha256: "8a3e7b1c2d5f4a6e8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a".into(),
          filename: Some("xiaomi-mt6877.bin".into()),
          size_bytes: Some(262144),
        }),
        auth: Some(pawflash_core::penumbra::manifest::FileBlob {
          url: "https://example.com/xiaomi-mt6877.auth".into(),
          sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
          filename: Some("xiaomi-mt6877.auth".into()),
          size_bytes: Some(4096),
        }),
        url: "https://example.com/xiaomi-mt6877.bin".into(),
        sha256: "8a3e7b1c2d5f4a6e8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a".into(),
        auth_url: Some("https://example.com/xiaomi-mt6877.auth".into()),
        auth_sha256: Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into()),
        verified: Some(true),
        notes: Some("Includes Xiaomi SLA/DAA Auth bypass".into()),
      },
      pawflash_core::penumbra::DAEntry {
        id: Some("tecno-mt6768".into()),
        brand: "tecno".into(),
        chipset: "mt6768".into(),
        devices: vec!["Tecno Spark 9 Pro".into(), "Tecno Camon 19".into()],
        da: Some(pawflash_core::penumbra::manifest::FileBlob {
          url: "https://example.com/tecno-mt6768.bin".into(),
          sha256: "1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d".into(),
          filename: Some("tecno-mt6768.bin".into()),
          size_bytes: Some(147456),
        }),
        auth: None,
        url: "https://example.com/tecno-mt6768.bin".into(),
        sha256: "1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d".into(),
        auth_url: None,
        auth_sha256: None,
        verified: Some(true),
        notes: None,
      },
      pawflash_core::penumbra::DAEntry {
        id: Some("oppo-mt6789".into()),
        brand: "oppo".into(),
        chipset: "mt6789".into(),
        devices: vec!["Oppo Reno 8T".into()],
        da: None,
        auth: None,
        url: "https://example.com/oppo-mt6789.bin".into(),
        sha256: "5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b".into(),
        auth_url: None,
        auth_sha256: None,
        verified: Some(true),
        notes: None,
      },
    ]);
  }
  tokio::task::spawn_blocking(pawflash_core::penumbra::list_dais)
    .await
    .map_err(|e| AppError::Other { message: e.to_string() })?
    .map_err(|e| AppError::Other { message: penumbra_err_string(&e) })
}

#[tracing::instrument(skip_all, fields(simulate))]
#[tauri::command]
async fn penumbra_da_download(
  device: Option<String>,
  on_event: Channel<ProgressEvent>,
  simulate: bool,
) -> Result<(), AppError> {
  send_progress(&on_event, ProgressEvent::PenumbraPhase { phase: "manifest".into(), message: "Fetching DA manifest...".into() });

  if simulate {
    send_progress(&on_event, ProgressEvent::PenumbraPhase { phase: "download".into(), message: "Downloading DA binary (simulated)...".into() });
    let _ = on_event.send(ProgressEvent::PenumbraProgress { bytes: 184320, total: 184320 });
    if let Some(ref d) = device
      && (d.to_lowercase().contains("redmi") || d.to_lowercase().contains("xiaomi"))
    {
      send_progress(&on_event, ProgressEvent::PenumbraPhase { phase: "download".into(), message: "Downloading companion Auth file (simulated)...".into() });
      let _ = on_event.send(ProgressEvent::PenumbraProgress { bytes: 4096, total: 4096 });
    }
    send_progress(&on_event, ProgressEvent::PenumbraDone { ok: true, detail: "DA & Auth combo installed (simulated)".into() });
    return Ok(());
  }

  let entry = match device {
    Some(d) if !d.trim().is_empty() => {
      pawflash_core::penumbra::resolve_by_device(&d)
        .map_err(|e| AppError::Other { message: penumbra_err_string(&e) })?
    }
    _ => return Err(AppError::Other { message: "enter a device model name".into() }),
  };

  send_progress(&on_event, ProgressEvent::PenumbraPhase {
    phase: "download".into(),
    message: format!("Downloading DA for {} ({})...", entry.brand, entry.chipset),
  });

  let existing_auth = pawflash_core::penumbra::load_selection().and_then(|s| s.auth_path);
  let channel_da = on_event.clone();
  let channel_auth = on_event.clone();
  let entry_clone = entry.clone();

  let (da_path, auth_path) = tokio::task::spawn_blocking(move || {
    let mut last_sent_da = 0u64;
    let mut on_da_progress = |done: u64, total: u64| {
      if done - last_sent_da >= 64 * 1024 || done == total {
        last_sent_da = done;
        let _ = channel_da.send(ProgressEvent::PenumbraProgress { bytes: done, total });
      }
    };
    let mut last_sent_auth = 0u64;
    let mut on_auth_progress = |done: u64, total: u64| {
      if done - last_sent_auth >= 1024 || done == total {
        last_sent_auth = done;
        let _ = channel_auth.send(ProgressEvent::PenumbraProgress { bytes: done, total });
      }
    };
    pawflash_core::penumbra::download_da_combo(&entry_clone, &mut on_da_progress, &mut on_auth_progress)
  })
  .await
  .map_err(|e| AppError::Other { message: e.to_string() })?
  .map_err(|e| AppError::Other { message: penumbra_err_string(&e) })?;

  let final_auth_path = auth_path
    .map(|p| p.display().to_string())
    .or(existing_auth);

  let da_sha256 = entry.da_sha256().to_string();
  let sel = pawflash_core::penumbra::DaSelection {
    brand: entry.brand,
    chipset: entry.chipset,
    path: da_path.display().to_string(),
    sha256: da_sha256,
    auth_path: final_auth_path,
    is_custom: false,
  };
  pawflash_core::penumbra::save_selection(&sel)
    .map_err(|e| AppError::Other { message: penumbra_err_string(&e) })?;

  let summary = if sel.auth_path.is_some() {
    format!("installed DA + Auth combo at {}", da_path.display())
  } else {
    format!("installed DA at {}", da_path.display())
  };
  send_progress(&on_event, ProgressEvent::PenumbraDone { ok: true, detail: summary });
  Ok(())
}

#[tracing::instrument(skip_all)]
#[tauri::command]
async fn penumbra_da_status() -> Result<pawflash_core::penumbra::DaSelection, AppError> {
  pawflash_core::penumbra::load_selection()
    .ok_or_else(|| AppError::Other { message: "no DA selected".into() })
}

#[tracing::instrument(skip_all)]
#[tauri::command]
async fn penumbra_da_remove(on_event: Channel<ProgressEvent>) -> Result<(), AppError> {
  pawflash_core::penumbra::remove_cached_da()
    .map_err(|e| AppError::Other { message: penumbra_err_string(&e) })?;
  pawflash_core::penumbra::clear_selection()
    .map_err(|e| AppError::Other { message: penumbra_err_string(&e) })?;
  send_progress(&on_event, ProgressEvent::PenumbraDone { ok: true, detail: "DA cache removed".into() });
  Ok(())
}

#[tracing::instrument(skip_all)]
#[tauri::command]
async fn penumbra_doctor(on_event: Channel<ProgressEvent>, simulate: bool) -> Result<(), AppError> {
  match pawflash_core::penumbra::load_selection() {
    Some(sel) => send_progress(&on_event, ProgressEvent::PenumbraPhase { phase: "da".into(), message: format!("DA selected: {} ({})", sel.brand, sel.chipset) }),
    None => send_progress(&on_event, ProgressEvent::Error { message: "no DA selected".into() }),
  }
  if !simulate {
    match pawflash_core::penumbra::fetch_da_manifest() {
      Ok(m) => send_progress(&on_event, ProgressEvent::PenumbraPhase { phase: "manifest".into(), message: format!("manifest reachable ({})", m.version) }),
      Err(e) => send_progress(&on_event, ProgressEvent::Error { message: penumbra_err_string(&e) }),
    }
    let visible = pawflash_core::udev::device_visible().await;
    send_progress(&on_event, ProgressEvent::PenumbraPhase { phase: "device".into(), message: if visible { "DA-capable device visible".into() } else { "no DA-capable device visible".into() } });
  }
  let hint = pawflash_core::platform::CURRENT.post_handshake_hint();
  if !hint.is_empty() {
    send_progress(&on_event, ProgressEvent::PenumbraPhase { phase: "hint".into(), message: hint.into() });
  }
  send_progress(&on_event, ProgressEvent::PenumbraDone { ok: true, detail: "doctor complete".into() });
  Ok(())
}

/// Run a blocking core penumbra op, translating events into `ProgressEvent`s.
async fn run_penumbra_op<F, T>(on_event: &Channel<ProgressEvent>, op: F) -> Result<T, AppError>
where
  F: FnOnce(&mut (dyn FnMut(&pawflash_core::penumbra::PenumbraEvent) + Send)) -> Result<T, pawflash_core::penumbra::PenumbraError>
    + Send
    + 'static,
  T: Send + 'static,
{
  let channel = on_event.clone();
  tokio::task::spawn_blocking(move || {
    let mut emit = move |ev: &pawflash_core::penumbra::PenumbraEvent| {
      let _ = channel.send(match ev {
        pawflash_core::penumbra::PenumbraEvent::Phase { phase, message } => {
          ProgressEvent::PenumbraPhase { phase: phase.clone(), message: message.clone() }
        }
        pawflash_core::penumbra::PenumbraEvent::Progress { bytes, total } => {
          ProgressEvent::PenumbraProgress { bytes: *bytes, total: *total }
        }
        pawflash_core::penumbra::PenumbraEvent::Log { level, message } => {
          ProgressEvent::PenumbraPhase { phase: "log".into(), message: format!("[{level}] {message}") }
        }
        pawflash_core::penumbra::PenumbraEvent::Done { ok, detail } => {
          ProgressEvent::PenumbraDone { ok: *ok, detail: detail.clone() }
        }
      });
    };
    op(&mut emit)
  })
  .await
  .map_err(|e| AppError::Other { message: e.to_string() })?
  .map_err(|e| AppError::Other { message: penumbra_err_string(&e) })
}

#[tracing::instrument(skip_all, fields(partition, simulate))]
#[tauri::command]
async fn penumbra_read(
  partition: String,
  file: String,
  on_event: Channel<ProgressEvent>,
  simulate: bool,
) -> Result<u64, AppError> {
  let da = gui_da_bytes(simulate)?;
  send_progress(&on_event, ProgressEvent::PenumbraPhase { phase: "read".into(), message: format!("Reading {partition} → {file}") });
  run_penumbra_op(&on_event, move |emit| {
    pawflash_core::penumbra::read_partition(&da, &partition, Path::new(&file), simulate, emit)
  })
  .await
}

#[tracing::instrument(skip_all, fields(partition, simulate))]
#[tauri::command]
async fn penumbra_write(
  partition: String,
  file: String,
  on_event: Channel<ProgressEvent>,
  simulate: bool,
) -> Result<u64, AppError> {
  let da = gui_da_bytes(simulate)?;
  if !simulate && !Path::new(&file).exists() {
    return Err(AppError::Other { message: format!("file not found: {file}") });
  }
  send_progress(&on_event, ProgressEvent::PenumbraPhase { phase: "write".into(), message: format!("Writing {file} → {partition}") });
  run_penumbra_op(&on_event, move |emit| {
    pawflash_core::penumbra::write_partition(&da, &partition, Path::new(&file), simulate, emit)
  })
  .await
}

#[tracing::instrument(skip_all, fields(partition, simulate))]
#[tauri::command]
async fn penumbra_erase(
  partition: String,
  on_event: Channel<ProgressEvent>,
  simulate: bool,
) -> Result<(), AppError> {
  let da = gui_da_bytes(simulate)?;
  send_progress(&on_event, ProgressEvent::PenumbraPhase { phase: "erase".into(), message: format!("Erasing {partition}") });
  run_penumbra_op(&on_event, move |emit| {
    pawflash_core::penumbra::erase_partition(&da, &partition, simulate, emit)
  })
  .await
}

#[tracing::instrument(skip_all, fields(unlock, simulate))]
#[tauri::command]
async fn penumbra_seccfg(
  unlock: bool,
  on_event: Channel<ProgressEvent>,
  simulate: bool,
) -> Result<(), AppError> {
  let da = gui_da_bytes(simulate)?;
  send_progress(&on_event, ProgressEvent::PenumbraPhase { phase: "seccfg".into(), message: if unlock { "Unlocking bootloader...".to_string() } else { "Locking bootloader...".to_string() } });
  run_penumbra_op(&on_event, move |emit| {
    pawflash_core::penumbra::seccfg(&da, unlock, simulate, emit)
  })
  .await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PenumbraPartitionInfo {
  pub name: String,
  pub address: u64,
  pub size: u64,
  pub size_formatted: String,
  pub section: String,
}

#[tracing::instrument(skip_all, fields(partition, simulate))]
#[tauri::command]
async fn penumbra_format(
  partition: String,
  on_event: Channel<ProgressEvent>,
  simulate: bool,
) -> Result<(), AppError> {
  let da = gui_da_bytes(simulate)?;
  send_progress(
    &on_event,
    ProgressEvent::PenumbraPhase {
      phase: "format".into(),
      message: format!("Formatting partition {partition}"),
    },
  );
  run_penumbra_op(&on_event, move |emit| {
    pawflash_core::penumbra::format(&da, &partition, simulate, emit)
  })
  .await
}

#[tracing::instrument(skip_all)]
#[tauri::command]
async fn penumbra_pgpt(
  on_event: Channel<ProgressEvent>,
  simulate: bool,
) -> Result<Vec<PenumbraPartitionInfo>, AppError> {
  let da = gui_da_bytes(simulate)?;
  if simulate {
    send_progress(&on_event, ProgressEvent::PenumbraDone { ok: true, detail: "pgpt (simulated)".into() });
    return Ok(vec![
      PenumbraPartitionInfo {
        name: "boot".into(),
        address: 0x1000,
        size: 64 * 1024 * 1024,
        size_formatted: "64.0 MiB".into(),
        section: "USER".into(),
      },
      PenumbraPartitionInfo {
        name: "recovery".into(),
        address: 0x5000,
        size: 64 * 1024 * 1024,
        size_formatted: "64.0 MiB".into(),
        section: "USER".into(),
      },
      PenumbraPartitionInfo {
        name: "nvram".into(),
        address: 0x9000,
        size: 5 * 1024 * 1024,
        size_formatted: "5.0 MiB".into(),
        section: "USER".into(),
      },
      PenumbraPartitionInfo {
        name: "userdata".into(),
        address: 0x10000,
        size: 32 * 1024 * 1024 * 1024,
        size_formatted: "32.0 GiB".into(),
        section: "USER".into(),
      },
    ]);
  }
  run_penumbra_op(&on_event, move |emit| {
    pawflash_core::penumbra::pgpt(&da, false, emit).map(|entries| {
      entries
        .into_iter()
        .map(|p| PenumbraPartitionInfo {
          size_formatted: pawflash_core::scatter_parser::human_size(p.size as i64),
          name: p.name,
          address: p.address,
          size: p.size,
          section: p.section,
        })
        .collect()
    })
  })
  .await
}

#[tracing::instrument(skip_all, fields(scatter_path, backup_protected, simulate))]
#[tauri::command]
async fn penumbra_flash_scatter(
  scatter_path: String,
  partitions: Option<Vec<String>>,
  backup_protected: bool,
  on_event: Channel<ProgressEvent>,
  simulate: bool,
) -> Result<(), AppError> {
  let da = gui_da_bytes(simulate)?;
  let auth = gui_auth_bytes(simulate)?;
  send_progress(
    &on_event,
    ProgressEvent::PenumbraPhase {
      phase: "scatter-flash".into(),
      message: format!("Flashing scatter: {scatter_path}"),
    },
  );
  run_penumbra_op(&on_event, move |emit| {
    pawflash_core::penumbra::flash_scatter(
      &da,
      auth.as_deref(),
      Path::new(&scatter_path),
      partitions.as_deref(),
      backup_protected,
      simulate,
      emit,
    )
  })
  .await
}

#[tracing::instrument(skip_all, fields(dir, simulate))]
#[tauri::command]
async fn penumbra_backup_calibration(
  dir: String,
  on_event: Channel<ProgressEvent>,
  simulate: bool,
) -> Result<Vec<String>, AppError> {
  let da = gui_da_bytes(simulate)?;
  let auth = gui_auth_bytes(simulate)?;
  send_progress(
    &on_event,
    ProgressEvent::PenumbraPhase {
      phase: "backup-calibration".into(),
      message: format!("Backing up NVRAM & calibration to {dir}"),
    },
  );
  run_penumbra_op(&on_event, move |emit| {
    pawflash_core::penumbra::backup_calibration(
      &da,
      auth.as_deref(),
      Path::new(&dir),
      simulate,
      emit,
    )
  })
  .await
}

#[tracing::instrument(skip_all, fields(simulate))]
#[tauri::command]
async fn penumbra_crash(on_event: Channel<ProgressEvent>, simulate: bool) -> Result<(), AppError> {
  let da = gui_da_bytes(simulate)?;
  send_progress(
    &on_event,
    ProgressEvent::PenumbraPhase {
      phase: "crash".into(),
      message: "Crashing device to bootrom...".into(),
    },
  );
  run_penumbra_op(&on_event, move |emit| {
    pawflash_core::penumbra::crash(&da, simulate, emit)
  })
  .await
}

#[tracing::instrument(skip_all, fields(path))]
#[tauri::command]
async fn penumbra_set_custom_da(path: String, auth_path: Option<String>) -> Result<(), AppError> {
  let p = Path::new(&path);
  if !p.is_file() {
    return Err(AppError::Other {
      message: format!("custom DA file not found: {path}"),
    });
  }
  let sel = pawflash_core::penumbra::DaSelection {
    brand: "custom".into(),
    chipset: "custom".into(),
    path,
    sha256: String::new(),
    auth_path,
    is_custom: true,
  };
  pawflash_core::penumbra::save_selection(&sel)
    .map_err(|e| AppError::Other { message: e.to_string() })
}

#[tracing::instrument(skip_all, fields(auth_path))]
#[tauri::command]
async fn penumbra_set_auth(auth_path: Option<String>) -> Result<(), AppError> {
  if let Some(mut sel) = pawflash_core::penumbra::load_selection() {
    sel.auth_path = auth_path;
    pawflash_core::penumbra::save_selection(&sel)
      .map_err(|e| AppError::Other { message: e.to_string() })?;
  }
  Ok(())
}

#[tracing::instrument(skip_all, fields(simulate))]
#[tauri::command]
async fn detect_device_mode(simulate: bool) -> Result<String, AppError> {
  if simulate {
    return Ok("fastboot".into());
  }
  if let Ok(executor) = FlashExecutor::connect().await {
    let _ = executor;
    return Ok("fastboot".into());
  }
  if let Some(mode) = pawflash_core::penumbra::detect_mtk_port() {
    return Ok(mode);
  }
  Ok("none".into())
}

#[tracing::instrument(skip_all, fields(mode, simulate))]
#[tauri::command]
async fn penumbra_reboot(
  mode: String,
  on_event: Channel<ProgressEvent>,
  simulate: bool,
) -> Result<(), AppError> {
  let da = gui_da_bytes(simulate)?;
  let boot = match mode.as_str() {
    "normal" => pawflash_core::penumbra::PenumbraBootMode::Normal,
    "homescreen" => pawflash_core::penumbra::PenumbraBootMode::HomeScreen,
    "fastboot" => pawflash_core::penumbra::PenumbraBootMode::Fastboot,
    "meta" => pawflash_core::penumbra::PenumbraBootMode::Meta,
    "test" => pawflash_core::penumbra::PenumbraBootMode::Test,
    other => return Err(AppError::Other { message: format!("invalid boot mode '{other}'") }),
  };
  run_penumbra_op(&on_event, move |emit| {
    pawflash_core::penumbra::reboot(&da, boot, simulate, emit)
  })
  .await
}

#[tracing::instrument(skip_all)]
#[tauri::command]
async fn penumbra_shutdown(on_event: Channel<ProgressEvent>, simulate: bool) -> Result<(), AppError> {
  let da = gui_da_bytes(simulate)?;
  if simulate {
    send_progress(&on_event, ProgressEvent::PenumbraDone { ok: true, detail: "shutdown (simulated)".into() });
    return Ok(());
  }
  run_penumbra_op(&on_event, move |emit| {
    pawflash_core::penumbra::shutdown(&da, false, emit)
  })
  .await
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
) -> Result<sp::FlashPlan, AppError> {
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
) -> Result<pawflash_core::flash::results::FlashResult, AppError> {
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
    return Err(format!("flash plan has {} error(s)", plan.errors.len()).into());
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
    send_progress(&on_event, ProgressEvent::Phase { phase: "connecting".into(), message: "Waiting for fastboot device...".into() });
  }
  let wait_token = fresh_cancel_token(&cancel);
  let mut executor = AnyExecutor::connect_wait(
    simulate,
    Some(parsed.as_ref()),
    std::time::Duration::from_secs(60),
    wait_token,
  )
  .await
  .map_err(|e| {
    warn!(error = %e, "execute_plan: connect failed");
    e
  })?;

  if executor.is_fastbootd().await {
    let msg = "device is in fastbootd mode (is-userspace = yes); scatter flashing requires bootloader mode. Run 'fastboot reboot bootloader' first.";
    warn!(%msg);
    send_progress(&on_event, ProgressEvent::Error { message: msg.into() });
    return Err(msg.into());
  }

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
    detail: format!(
      "{} succeeded, {} failed of {} partitions",
      result.succeeded, result.failed, result.total
    ),
  });

  Ok(result)
}

#[tracing::instrument(skip(cancel))]
#[tauri::command]
async fn cancel_flash(cancel: State<'_, CancelState>) -> Result<(), AppError> {
  info!("cancel_flash requested");
  cancel.flash.store(true, Ordering::Relaxed);
  cancel
    .cancel_token
    .lock()
    .unwrap_or_else(|p| p.into_inner())
    .cancel();
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
) -> Result<String, AppError> {
  let _guard = OpGuard::new(&cancel)?;
  info!(%partition, %image_path, simulate, "flash_raw_image started");
  send_progress(&on_event, ProgressEvent::Phase { phase: "connecting".into(), message: "Waiting for fastboot device...".into() });
  if simulate {
    send_progress(&on_event, ProgressEvent::Warning { message: "SIMULATED MODE — no device will be touched".into() });
  }
  let wait_token = fresh_cancel_token(&cancel);
  let mut executor = AnyExecutor::connect_wait(
    simulate,
    None,
    std::time::Duration::from_secs(60),
    wait_token,
  )
  .await
  .map_err(|e| {
    warn!(error = %e, "connect failed");
    send_progress(&on_event, ProgressEvent::Error { message: format!("Device connection failed: {e}") });
    e
  })?;

  let path = Path::new(&image_path);
  if !path.exists() {
    warn!(%image_path, "image not found");
    send_progress(&on_event, ProgressEvent::Error { message: format!("Image not found: {image_path}") });
    return Err(format!("image not found: {image_path}").into());
  }

  let metadata = tokio::fs::metadata(path).await.map_err(|e| {
    warn!(%image_path, error = %e, "failed to stat image");
    send_progress(&on_event, ProgressEvent::Error { message: format!("Failed to stat image: {e}") });
    format!("failed to read image metadata: {e}")
  })?;
  let file_size = metadata.len();
  info!(%partition, %image_path, file_size, "image verified");
  send_progress(
    &on_event,
    ProgressEvent::DeviceAction {
      action: "image_info".into(),
      detail: format!("Image: {} ({} bytes)", path.file_name().unwrap_or_default().to_string_lossy(), file_size),
    },
  );

  let current_slot = executor.device_vars().get("current-slot").cloned();
  let has_slot_key = format!("has-slot:{partition}");
  let has_slot = executor.device_vars().get(&has_slot_key).cloned();
  let unlocked = executor.device_vars().get("unlocked").cloned();
  let is_userspace = executor.device_vars().get("is-userspace").cloned();

  info!(
    partition = %partition,
    current_slot = current_slot.as_deref().unwrap_or("none"),
    has_slot = has_slot.as_deref().unwrap_or("unknown"),
    unlocked = unlocked.as_deref().unwrap_or("unknown"),
    is_userspace = is_userspace.as_deref().unwrap_or("unknown"),
    "device status before raw flash"
  );
  send_progress(
    &on_event,
    ProgressEvent::DeviceAction {
      action: "device_status".into(),
      detail: format!(
        "Mode: {} | Unlocked: {} | CurrentSlot: {} | HasSlot: {}",
        if is_userspace.as_deref() == Some("yes") { "fastbootd" } else { "bootloader" },
        unlocked.as_deref().unwrap_or("?"),
        current_slot.as_deref().unwrap_or("none"),
        has_slot.as_deref().unwrap_or("?"),
      ),
    },
  );

  if is_userspace.as_deref() == Some("yes") || is_userspace.as_deref() == Some("true") {
    let msg = "device is in fastbootd mode (is-userspace = yes); flashing requires bootloader mode. Run 'fastboot reboot bootloader' first.";
    warn!(%msg);
    send_progress(&on_event, ProgressEvent::Error { message: msg.into() });
    return Err(msg.into());
  }

  if unlocked.as_deref() == Some("no") {
    warn!(%partition, "bootloader is reported locked (unlocked: no)");
    send_progress(
      &on_event,
      ProgressEvent::Warning {
        message: "Device bootloader reports locked (unlocked: no). Flash may be rejected by device.".into(),
      },
    );
  }

  // Resolve the target partition:
  // 1. If explicit _a/_b suffix provided, use verbatim.
  // 2. If device reports has-slot:<partition> == "no", use bare partition.
  // 3. If device has current-slot ("a" or "b"), append current slot suffix.
  // 4. Otherwise use bare partition name.
  let target = if partition.ends_with("_a") || partition.ends_with("_b") {
    partition.clone()
  } else if has_slot.as_deref() == Some("no") {
    info!(%partition, "partition explicitly reported as non-A/B (has-slot: no)");
    partition.clone()
  } else if let Some(ref slot) = current_slot {
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
    return Err(format!("{target} is a {role} partition; refusing without explicit confirmation").into());
  }
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

  let resp = match executor
    .flash_raw_image_with_callback(&target, path, Some(&mut on_transfer))
    .await
  {
    Ok(r) => r,
    Err(e) => {
      let err_str = e.to_string();
      let not_found = err_str.contains("not found")
        || err_str.contains("No such partition")
        || err_str.contains("Partition doesn't exist")
        || err_str.contains("does not exist")
        || err_str.contains("not exist");

      if not_found && target != partition {
        warn!(%target, fallback = %partition, error = %e, "target with slot suffix failed with not found; retrying with bare partition");
        send_progress(
          &on_event,
          ProgressEvent::Warning {
            message: format!("Flash {target} rejected ({err_str}). Retrying bare partition {partition}..."),
          },
        );
        match executor
          .flash_raw_image_with_callback(&partition, path, Some(&mut on_transfer))
          .await
        {
          Ok(r) => r,
          Err(fallback_err) => {
            warn!(%partition, error = %fallback_err, "fallback flash also failed");
            send_progress(
              &on_event,
              ProgressEvent::Error {
                message: format!("Flash {partition} failed: {fallback_err}"),
              },
            );
            return Err(fallback_err.to_string().into());
          }
        }
      } else {
        warn!(%target, error = %e, "flash_raw_image failed");
        send_progress(
          &on_event,
          ProgressEvent::Error {
            message: format!("Flash {target} failed: {e}"),
          },
        );
        return Err(e.to_string().into());
      }
    }
  };

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

/// Opens an HTTP/HTTPS URL in the default system browser.
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
  if !url.starts_with("https://") && !url.starts_with("http://") {
    return Err("Invalid URL protocol".into());
  }

  info!(url = %url, "opening external url in system browser");

  #[cfg(target_os = "windows")]
  let mut cmd = {
    let mut c = std::process::Command::new("rundll32");
    c.args(["url.dll,FileProtocolHandler", &url]);
    c
  };

  #[cfg(target_os = "macos")]
  let mut cmd = {
    let mut c = std::process::Command::new("open");
    c.arg(&url);
    c
  };

  #[cfg(not(any(target_os = "windows", target_os = "macos")))]
  let mut cmd = {
    let mut c = std::process::Command::new("xdg-open");
    c.arg(&url);
    c
  };

  cmd.spawn().map_err(|e| {
    warn!(url = %url, error = %e, "failed to spawn browser process");
    format!("Failed to open URL: {e}")
  })?;

  Ok(())
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
      get_all_vars,
      disable_vbmeta,
      parse_scatter,
      build_plan,
      execute_plan,
      cancel_flash,
      flash_raw_image,
      classify_partition,
      mtk_status,
      mtk_download,
      mtk_remove,
      mtk_doctor,
      mtk_read,
      mtk_write,
      mtk_erase,
      penumbra_status,
      penumbra_list_devices,
      penumbra_da_download,
      penumbra_da_status,
      penumbra_da_remove,
      penumbra_doctor,
      penumbra_read,
      penumbra_write,
      penumbra_erase,
      penumbra_format,
      penumbra_seccfg,
      penumbra_pgpt,
      penumbra_flash_scatter,
      penumbra_backup_calibration,
      penumbra_crash,
      penumbra_set_custom_da,
      penumbra_set_auth,
      detect_device_mode,
      penumbra_reboot,
      penumbra_shutdown,
      shutdown_device,
      open_url,
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

    #[test]
    fn app_error_serializes_as_tagged_dto() {
        // The wire shape must match the TS `AppError` mirror in
        // `src/types/api.ts` (`{ kind, detail: { ... } }`).
        let value =
            serde_json::to_value(AppError::ActionFailed { partition: "boot".into(), message: "boom".into() })
                .expect("serializes");
        assert_eq!(value["kind"], "ActionFailed");
        assert_eq!(value["detail"]["partition"], "boot");
        assert_eq!(value["detail"]["message"], "boom");

        let no_device =
            serde_json::to_value(AppError::NoDevice { message: "nope".into() }).expect("serializes");
        assert_eq!(no_device["kind"], "NoDevice");
        assert_eq!(no_device["detail"]["message"], "nope");
    }

    #[test]
    fn parttype_from_str_maps_valid_values() {
        assert_eq!(parttype_from_str("user").unwrap(), pawflash_core::mtk::PartType::User);
        assert_eq!(parttype_from_str("boot1").unwrap(), pawflash_core::mtk::PartType::Boot1);
        assert_eq!(parttype_from_str("boot2").unwrap(), pawflash_core::mtk::PartType::Boot2);
        assert_eq!(parttype_from_str("rpmb").unwrap(), pawflash_core::mtk::PartType::Rpmb);
        assert!(parttype_from_str("bogus").is_err());
    }

    #[test]
    fn mtk_progress_events_serialize_with_tag() {
        let ev = ProgressEvent::MtkProgress { bytes: 1024, total: 4096 };
        let v = serde_json::to_value(ev).unwrap();
        assert_eq!(v["event"], "MtkProgress");
        assert_eq!(v["data"]["bytes"], 1024);
    }

    #[test]
    fn penumbra_progress_events_serialize_with_tag() {
        let ev = ProgressEvent::PenumbraProgress { bytes: 1024, total: 4096 };
        let v = serde_json::to_value(ev).unwrap();
        assert_eq!(v["event"], "PenumbraProgress");
        assert_eq!(v["data"]["bytes"], 1024);
    }

    #[test]
    fn penumbra_simulate_read_uses_simulated_runner() {
        // Simulate mode must not require a DA selection: the empty-slice path
        // routes to the simulated runner, which emits a complete event stream.
        let mut events = Vec::new();
        let bytes = pawflash_core::penumbra::read_partition(
            &[],
            "boot",
            Path::new("/tmp/boot.img"),
            true, // simulate
            &mut |e| events.push(e.clone()),
        )
        .expect("simulated read succeeds");
        assert_eq!(bytes, 128 * 1024 * 1024);
        assert!(matches!(
            events.last(),
            Some(pawflash_core::penumbra::PenumbraEvent::Done { ok: true, .. })
        ));
    }

    #[test]
    fn mtk_simulate_read_uses_simulated_runner() {
        // Simulate mode must not require a manifest with real assets or an
        // installed bridge: the dummy-manifest path resolves and the
        // simulated runner emits a complete event stream.
        let manifest = pawflash_core::mtk::Manifest {
            version: "simulated".into(),
            commit: String::new(),
            platforms: HashMap::new(),
        };
        let mut events = Vec::new();
        let bytes = pawflash_core::mtk::read_partition(
            &manifest,
            "boot",
            Path::new("/tmp/boot.img"),
            pawflash_core::mtk::PartType::User,
            true, // simulate
            &mut |e| events.push(e.clone()),
        )
        .expect("simulated read succeeds");
        assert_eq!(bytes, 128 * 1024 * 1024);
        assert!(
            matches!(events.first(), Some(pawflash_core::mtk::MtkEvent::Phase { phase, .. })
                if phase == "connect")
        );
        assert!(
            events.iter().any(|e| matches!(
                e,
                pawflash_core::mtk::MtkEvent::Result { ok: true, .. }
            ))
        );
    }

    #[test]
    fn open_url_rejects_non_http_protocol() {
        assert!(open_url("javascript:alert(1)".into()).is_err());
        assert!(open_url("file:///etc/passwd".into()).is_err());
    }
}
