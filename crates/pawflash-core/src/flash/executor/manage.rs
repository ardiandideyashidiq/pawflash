use std::collections::HashMap;

use tracing::{debug, info};

use crate::flash::error::Result;
use crate::flash::transport::FlashTransport;
use super::{BootTarget, FlashExecutor};

impl<T: FlashTransport> FlashExecutor<T> {
    /// # Errors
    /// Returns an error if the device does not respond.
    pub async fn get_var(&mut self, var: &str) -> Result<String> {
        let t0 = std::time::Instant::now();
        let res = self.fb.get_var(var).await;
        let d = t0.elapsed();
        debug!(var, ?d, "get_var finished");
        res
    }

    /// # Errors
    /// Returns an error if the device does not respond within the timeout.
    pub async fn get_all_vars(&mut self) -> Result<HashMap<String, String>> {
        let t0 = std::time::Instant::now();
        let res = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.fb.get_all_vars(),
        )
            .await
            .map_err(|_| crate::flash::error::FlashError::NoDevice)?;
        let d = t0.elapsed();
        info!(?d, count = res.as_ref().map_or(0, HashMap::len), "get_all_vars completed");
        res
    }

    /// # Errors
    /// Returns an error if the reboot command fails.
    pub async fn reboot(&mut self) -> Result<()> {
        let t0 = std::time::Instant::now();
        let res = self.fb.reboot().await;
        let d = t0.elapsed();
        info!(?d, "reboot completed");
        res
    }

    /// # Errors
    /// Returns an error if the reboot command fails.
    pub async fn reboot_to(&mut self, target: BootTarget) -> Result<()> {
        let t0 = std::time::Instant::now();
        let res = match target {
            BootTarget::System => self.fb.reboot().await,
            _ => self.fb.reboot_to(target.as_str()).await,
        };
        let d = t0.elapsed();
        info!(?target, ?d, "reboot_to completed");
        res
    }

    /// # Errors
    /// Returns an error if the flashing command fails.
    pub async fn flashing_lock(&mut self) -> Result<String> {
        let t0 = std::time::Instant::now();
        let res = match self.fb.flashing("lock").await {
            Ok(resp) => Ok(resp),
            Err(e) => {
                let err_str = e.to_string().to_ascii_lowercase();
                if err_str.contains("unknown") || err_str.contains("not recognized") {
                    self.fb.oem("lock").await
                } else {
                    Err(e)
                }
            }
        };
        let d = t0.elapsed();
        info!(?d, "flashing_lock completed");
        res
    }

    /// # Errors
    /// Returns an error if the flashing command fails.
    pub async fn flashing_unlock(&mut self) -> Result<String> {
        let t0 = std::time::Instant::now();
        let res = match self.fb.flashing("unlock").await {
            Ok(resp) => Ok(resp),
            Err(e) => {
                let err_str = e.to_string().to_ascii_lowercase();
                if err_str.contains("unknown") || err_str.contains("not recognized") {
                    self.fb.oem("unlock").await
                } else {
                    Err(e)
                }
            }
        };
        let d = t0.elapsed();
        info!(?d, "flashing_unlock completed");
        res
    }

    /// # Errors
    /// Returns an error if the `set_active` command fails.
    pub async fn set_active_slot(&mut self, slot: &str) -> Result<String> {
        let t0 = std::time::Instant::now();
        let res = self.fb.set_active(slot).await;
        let d = t0.elapsed();
        info!(slot, ?d, "set_active_slot completed");
        res
    }

}
