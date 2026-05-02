//! Binary entrypoint for the local Codex App update manager.

mod app;
mod builder;
mod cli;
mod codex_cli;
mod config;
mod install;
mod liveness;
mod logging;
mod notify;
mod state;
#[cfg(test)]
mod test_util;
mod upstream;

use anyhow::Result;
use clap::Parser;

#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[tokio::main]
async fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    app::run(cli).await
}
