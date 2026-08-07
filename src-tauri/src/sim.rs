//! Simulated fastboot executor for the GUI.
//!
//! Mirrors the CLI's `--simulate` flag: every device-touching operation can
//! run against a [`SimulatedTransport`] instead of real USB hardware. Image
//! files are still read from disk by the executor; only the USB transfer and
//! flash write phases are replaced with timed delays.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use pawflash_core::flash::executor::{BootTarget, FlashExecutor};
use pawflash_core::flash::progress::FlashRunOptions;
use pawflash_core::flash::results::FlashResult;
use pawflash_core::flash::simulate::{simulated_vars, SimulatedTransport};
use pawflash_core::scatter_parser::types::ScatterFile;
use tokio_util::sync::CancellationToken;

/// A flash executor backed by either a real USB device or a simulation.
///
/// Tauri commands cannot be generic (they are registered through
/// `generate_handler`), so the GUI dispatches through this enum instead.
pub enum AnyExecutor {
  Real(FlashExecutor<fastboot_protocol::nusb::NusbFastBoot>),
  Sim(FlashExecutor<SimulatedTransport>),
}

impl AnyExecutor {
  /// Connect to a device, or build a simulation when `simulate` is true.
  ///
  /// When `scatter` is provided, simulated device variables are seeded from
  /// it so `partition-type:`/`partition-size:` lookups match the firmware.
  ///
  /// # Errors
  ///
  /// Returns an error if connecting to a real device fails.
  pub async fn connect(simulate: bool, scatter: Option<&ScatterFile>) -> Result<Self, String> {
    if simulate {
      let transport = match scatter {
        Some(parsed) => SimulatedTransport::from_scatter(parsed),
        None => SimulatedTransport::new(simulated_vars()),
      };
      let vars = transport.device_vars().clone();
      Ok(Self::Sim(FlashExecutor::new(transport, vars)))
    } else {
      FlashExecutor::connect()
        .await
        .map(Self::Real)
        .map_err(|e| e.to_string())
    }
  }

  /// Like [`Self::connect`], but for real (non-simulated) connections it
  /// polls for up to `timeout` waiting for a fastboot device to appear —
  /// matching the CLI's flash connect behavior. Cancel aborts the wait.
  ///
  /// # Errors
  ///
  /// Returns an error if no device appears within `timeout` or the wait is
  /// cancelled.
  pub async fn connect_wait(
    simulate: bool,
    scatter: Option<&ScatterFile>,
    timeout: Duration,
    cancel: CancellationToken,
  ) -> Result<Self, String> {
    if simulate {
      Self::connect(simulate, scatter).await
    } else {
      FlashExecutor::wait_for_device(timeout, cancel)
        .await
        .map(Self::Real)
        .map_err(|e| e.to_string())
    }
  }

  #[must_use]
  pub fn device_vars(&self) -> &HashMap<String, String> {
    match self {
      Self::Real(executor) => executor.device_vars(),
      Self::Sim(executor) => executor.device_vars(),
    }
  }

  /// # Errors
  /// Returns an error if the device does not respond.
  pub async fn get_var(&mut self, var: &str) -> Result<String, String> {
    let result = match self {
      Self::Real(executor) => executor.get_var(var).await,
      Self::Sim(executor) => executor.get_var(var).await,
    };
    result.map_err(|e| e.to_string())
  }

  /// # Errors
  /// Returns an error if the reboot command fails.
  pub async fn reboot_to(&mut self, target: BootTarget) -> Result<(), String> {
    let result = match self {
      Self::Real(executor) => executor.reboot_to(target).await,
      Self::Sim(executor) => executor.reboot_to(target).await,
    };
    result.map_err(|e| e.to_string())
  }

  /// # Errors
  /// Returns an error if the flashing command fails.
  pub async fn flashing_lock(&mut self) -> Result<String, String> {
    let result = match self {
      Self::Real(executor) => executor.flashing_lock().await,
      Self::Sim(executor) => executor.flashing_lock().await,
    };
    result.map_err(|e| e.to_string())
  }

  /// # Errors
  /// Returns an error if the flashing command fails.
  pub async fn flashing_unlock(&mut self) -> Result<String, String> {
    let result = match self {
      Self::Real(executor) => executor.flashing_unlock().await,
      Self::Sim(executor) => executor.flashing_unlock().await,
    };
    result.map_err(|e| e.to_string())
  }

  /// # Errors
  /// Returns an error if the `set_active` command fails.
  pub async fn set_active_slot(&mut self, slot: &str) -> Result<String, String> {
    let result = match self {
      Self::Real(executor) => executor.set_active_slot(slot).await,
      Self::Sim(executor) => executor.set_active_slot(slot).await,
    };
    result.map_err(|e| e.to_string())
  }

  /// # Errors
  /// Returns an error if flashing the empty vbmeta fails.
  pub async fn flash_empty_vbmeta(&mut self) -> Result<String, String> {
    let result = match self {
      Self::Real(executor) => executor.flash_empty_vbmeta().await,
      Self::Sim(executor) => executor.flash_empty_vbmeta().await,
    };
    result.map_err(|e| e.to_string())
  }

  /// # Errors
  /// Returns an error if the image cannot be read or the flash fails.
  pub async fn flash_raw_image(&mut self, partition: &str, image_path: &Path) -> Result<String, String> {
    let result = match self {
      Self::Real(executor) => executor.flash_raw_image(partition, image_path).await,
      Self::Sim(executor) => executor.flash_raw_image(partition, image_path).await,
    };
    result.map_err(|e| e.to_string())
  }

  /// Execute a flash plan, streaming transfer progress through `opts`.
  pub async fn execute_plan(
    &mut self,
    plan: &pawflash_core::scatter_parser::types::FlashPlan,
    opts: FlashRunOptions<'_>,
  ) -> FlashResult {
    match self {
      Self::Real(executor) => executor.execute_plan(plan, opts).await,
      Self::Sim(executor) => executor.execute_plan(plan, opts).await,
    }
  }
}
