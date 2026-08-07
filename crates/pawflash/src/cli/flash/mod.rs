pub(crate) mod scatter;
pub(crate) mod raw;

use std::path::{Path, PathBuf};

use miette::{IntoDiagnostic, Result};
use clap::CommandFactory;
use tracing::warn;

use crate::cli::args::{Cli, FlashAction};
use pawflash_core::output;
use pawflash_core::scatter_parser as sp;

/// What action to perform with the scatter file.
#[derive(Debug, Clone, Copy)]
enum Action {
    /// Show scatter metadata (replaces `show` + `full_json`).
    Show { full_json: bool },
    /// Dry-run: print plan without executing.
    DryRun,
    /// Execute the flash plan.
    Execute,
}

/// Grouped config for scatter operations.
struct ScatterConfig<'a> {
    scatter_path: &'a Path,
    action: Action,
    options: sp::FlashPlanOptions,
    json: bool,
    simulate: bool,
}

/// Build shared flash plan options from parsed CLI args.
///
/// Both the interactive and the direct execution paths must use identical
/// options, otherwise flags such as `--include-preloader` silently diverge.
fn build_flash_options(
    scatter_path: &Path,
    storage: sp::StorageSelect,
    exclude: &[String],
    firmware_dir: Option<&Path>,
    image_verification: sp::ImageVerification,
    allowance: sp::Allowance,
) -> sp::FlashPlanOptions {
    sp::FlashPlanOptions {
        storage,
        exclude: exclude.to_vec(),
        firmware_dir: firmware_dir.map(Path::to_path_buf),
        package_root: Some(
            scatter_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_path_buf(),
        ),
        image_verification,
        allowance,
    }
}

fn print_flash_help() -> Result<()> {
    let mut cmd = Cli::command();
    if let Some(flash) = cmd.find_subcommand_mut("flash") {
        flash.print_help().into_diagnostic()?;
        output::status::blank();
    }
    Ok(())
}

/// Unified handler for all `pawflash flash` operations.
///
/// When `simulate` is true, uses [`SimulatedTransport`] instead of real
/// USB — scatter flash still reads image files from disk for realistic
/// I/O timing.
///
/// # Errors
///
/// Returns an error if the scatter file cannot be parsed, the device
/// is not reachable, or any flash operation fails.
pub async fn run(
    action: Option<FlashAction>,
    partition: Option<String>,
    image: Option<PathBuf>,
    slot: Option<String>,
    both: bool,
    force: bool,
    simulate: bool,
) -> Result<()> {
    match action {
        Some(FlashAction::Scatter {
            ref path,
            show,
            full_json,
            dry_run,
            json,
            storage,
            ref exclude,
            ref firmware_dir,
            check_images,
            include_preloader,
            image_search,
            allow_incomplete_slots,
        }) => {
            let Some(p) = path else {
                print_flash_help()?;
                return Ok(());
            };
            let scatter_path = p.clone();

            let options = build_flash_options(
                &scatter_path,
                storage,
                exclude,
                firmware_dir.as_deref(),
                sp::ImageVerification {
                    check_images,
                    image_search,
                },
                sp::Allowance {
                    include_preloader,
                    allow_incomplete_slots,
                },
            );

            // `--simulate` never touches real hardware, so `--json --simulate`
            // is a safe headless simulated run; the guard only protects the
            // real-device execute path.
            if json && !dry_run && !show && !simulate {
                miette::bail!("--json requires --dry-run (plan preview as JSON); refusing to flash");
            }

            if !show && !dry_run && !json {
                if !simulate {
                    warn!("no --json/--dry-run specified; entering interactive confirmation flow");
                }
                return crate::cli::interactive::run(&scatter_path, &options, simulate).await;
            }

            let action = if show {
                Action::Show { full_json }
            } else if dry_run {
                Action::DryRun
            } else {
                Action::Execute
            };
            let cfg = ScatterConfig {
                scatter_path: &scatter_path,
                action,
                options,
                json,
                simulate,
            };
            scatter::run_scatter(&cfg).await?;
        }

        None => {
            let Some(partition) = partition else {
                print_flash_help()?;
                return Ok(());
            };
            let Some(image) = image else {
                print_flash_help()?;
                return Ok(());
            };
            raw::run_raw_image(&partition, &image, slot, both, force, simulate).await?;
        }
    }

    Ok(())
}
