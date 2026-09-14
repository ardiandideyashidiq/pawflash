use std::collections::HashMap;

use crate::flash::error::Result;
use crate::flash::transport::FlashTransport;
use super::{BootTarget, FlashExecutor};

impl<T: FlashTransport> FlashExecutor<T> {
    /// # Errors
    /// Returns an error if the device does not respond.
    pub async fn get_var(&mut self, var: &str) -> Result<String> {
        self.fb.get_var(var).await
    }

    /// # Errors
    /// Returns an error if the device does not respond within the timeout.
    pub async fn get_all_vars(&mut self) -> Result<HashMap<String, String>> {
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.fb.get_all_vars(),
        )
            .await
            .map_err(|_| crate::flash::error::FlashError::NoDevice)?
    }

    /// # Errors
    /// Returns an error if the reboot command fails.
    pub async fn reboot(&mut self) -> Result<()> {
        self.fb.reboot().await
    }

    /// # Errors
    /// Returns an error if the reboot command fails.
    pub async fn reboot_to(&mut self, target: BootTarget) -> Result<()> {
        match target {
            BootTarget::System => self.fb.reboot().await,
            _ => self.fb.reboot_to(target.as_str()).await,
        }
    }

    /// # Errors
    /// Returns an error if the flashing command fails.
    pub async fn flashing_lock(&mut self) -> Result<String> {
        match self.fb.flashing("lock").await {
            Ok(resp) => Ok(resp),
            Err(e) => {
                let err_str = e.to_string().to_ascii_lowercase();
                if err_str.contains("unknown") || err_str.contains("not recognized") {
                    self.fb.oem("lock").await
                } else {
                    Err(e)
                }
            }
        }
    }

    /// # Errors
    /// Returns an error if the flashing command fails.
    pub async fn flashing_unlock(&mut self) -> Result<String> {
        match self.fb.flashing("unlock").await {
            Ok(resp) => Ok(resp),
            Err(e) => {
                let err_str = e.to_string().to_ascii_lowercase();
                if err_str.contains("unknown") || err_str.contains("not recognized") {
                    self.fb.oem("unlock").await
                } else {
                    Err(e)
                }
            }
        }
    }

    /// # Errors
    /// Returns an error if the `set_active` command fails.
    pub async fn set_active_slot(&mut self, slot: &str) -> Result<String> {
        self.fb.set_active(slot).await
    }

}
