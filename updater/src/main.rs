//! Binary entrypoint for the local Codex App update manager.

#[cfg(unix)]
mod app;
#[cfg(unix)]
mod builder;
#[cfg(unix)]
mod cache_cleanup;
#[cfg(unix)]
mod changelog;
mod cli;
#[cfg(unix)]
mod codex_cli;
#[cfg(unix)]
mod config;
#[cfg(unix)]
mod dmg_source;
#[cfg(unix)]
mod feature_picker;
mod install;
#[cfg(unix)]
mod install_rollback;
#[cfg(unix)]
mod liveness;
#[cfg(unix)]
mod logging;
#[cfg(unix)]
mod notify;
#[cfg(unix)]
mod package_verification;
#[cfg(unix)]
mod redaction;
#[cfg(unix)]
mod rollback;
#[cfg(unix)]
mod state;
#[cfg(all(test, unix))]
mod test_util;
#[cfg(unix)]
mod trust;
#[cfg(unix)]
mod wrapper;
#[cfg(unix)]
mod wrapper_apply;

#[cfg(unix)]
use anyhow::Result;
#[cfg(unix)]
use clap::Parser;

#[cfg(all(test, unix))]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(unix)]
#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    app::run(cli).await
}

#[cfg(not(unix))]
fn main() {
    eprintln!("codex-app-updater supports Unix-like Linux package hosts only.");
    std::process::exit(1);
}
