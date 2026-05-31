//! Command-line interface definition for the updater binary.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "codex-app-updater")]
#[command(about = "Local update manager for Codex App on Linux")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
/// Top-level commands supported by the updater binary.
pub enum Commands {
    Daemon,
    CheckNow {
        #[arg(long, default_value_t = false)]
        if_stale: bool,
    },
    /// Check whether a newer codex-app wrapper release is available, and record
    /// its changelog.
    CheckWrapper {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Apply the recorded wrapper update candidate for the running install.
    ApplyWrapperUpdate,
    /// Show a GUI checklist of optional port integrations and save the
    /// selection so the next wrapper rebuild honors it.
    /// Invoked by the in-app Update button at click time (display still alive).
    #[command(name = "pick-integrations", alias = "pick-features")]
    PickFeatures {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    CliPreflight {
        #[arg(long)]
        cli_path: Option<PathBuf>,
        #[arg(long)]
        print_path: bool,
        #[arg(long, default_value_t = false)]
        allow_install_missing: bool,
    },
    PromptInstallCli {
        #[arg(long)]
        cli_path: Option<PathBuf>,
        #[arg(long)]
        print_path: bool,
    },
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Install the already rebuilt update package, if one is ready.
    InstallReady,
    /// Roll back to the last retained known-good package.
    Rollback,
    /// Install a Debian package (.deb) with elevated privileges.
    InstallDeb {
        #[arg(long)]
        path: PathBuf,
        #[arg(long, hide = true)]
        expected_sha256: Option<String>,
        #[arg(long, hide = true)]
        expected_package_name: Option<String>,
        #[arg(long, hide = true)]
        expected_package_version: Option<String>,
        #[arg(long, hide = true)]
        allow_same_version: bool,
    },
    /// Install an RPM package (.rpm) with elevated privileges.
    InstallRpm {
        #[arg(long)]
        path: PathBuf,
        #[arg(long, hide = true)]
        expected_sha256: Option<String>,
        #[arg(long, hide = true)]
        expected_package_name: Option<String>,
        #[arg(long, hide = true)]
        expected_package_version: Option<String>,
        #[arg(long, hide = true)]
        allow_same_version: bool,
    },
    /// Install a pacman package (.pkg.tar.zst) with elevated privileges.
    InstallPacman {
        #[arg(long)]
        path: PathBuf,
        #[arg(long, hide = true)]
        expected_sha256: Option<String>,
        #[arg(long, hide = true)]
        expected_package_name: Option<String>,
        #[arg(long, hide = true)]
        expected_package_version: Option<String>,
        #[arg(long, hide = true)]
        allow_same_version: bool,
    },
    /// Install a Debian package as an explicit rollback with elevated privileges.
    InstallRollbackDeb {
        #[arg(long)]
        path: PathBuf,
        #[arg(long, hide = true)]
        expected_sha256: Option<String>,
        #[arg(long, hide = true)]
        expected_package_name: Option<String>,
        #[arg(long, hide = true)]
        expected_package_version: Option<String>,
    },
    /// Install an RPM package as an explicit rollback with elevated privileges.
    InstallRollbackRpm {
        #[arg(long)]
        path: PathBuf,
        #[arg(long, hide = true)]
        expected_sha256: Option<String>,
        #[arg(long, hide = true)]
        expected_package_name: Option<String>,
        #[arg(long, hide = true)]
        expected_package_version: Option<String>,
    },
    /// Install a pacman package as an explicit rollback with elevated privileges.
    InstallRollbackPacman {
        #[arg(long)]
        path: PathBuf,
        #[arg(long, hide = true)]
        expected_sha256: Option<String>,
        #[arg(long, hide = true)]
        expected_package_name: Option<String>,
        #[arg(long, hide = true)]
        expected_package_version: Option<String>,
    },
}
