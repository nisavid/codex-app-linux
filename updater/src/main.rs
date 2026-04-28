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
mod package_version;
mod state;
mod upstream;

use clap::Parser;

#[tokio::main]
async fn main() {
    let cli = cli::Cli::parse();
    if let Err(error) = app::run(cli).await {
        eprintln!("Error: {error:?}");
        if is_configured_cli_path_error(&error) {
            std::process::exit(78);
        }
        std::process::exit(1);
    }
}

fn is_configured_cli_path_error(error: &anyhow::Error) -> bool {
    crate::codex_cli::is_invalid_configured_cli_path_error(error)
}
