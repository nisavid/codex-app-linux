//! Application entrypoints and orchestration for the local updater daemon.

use crate::{
    builder,
    cli::{Cli, Commands},
    codex_cli,
    config::{RuntimeConfig, RuntimePaths},
    install, liveness, logging, notify, package_version,
    state::{CliStatus, PersistedState, UpdateStatus},
    upstream,
};
use anyhow::{Context, Result};
use chrono::{Duration as ChronoDuration, Utc};
use fs4::{FileExt, TryLockError};
use reqwest::Client;
use std::{
    fs::{self, OpenOptions},
    io::{Seek, SeekFrom, Write},
    path::Path,
};
use tokio::time::{self, Duration};
use tracing::{error, info, warn};

const RECONCILE_INTERVAL_SECONDS: u64 = 15;
const CLI_MISSING_NOTIFICATION_EVENT: &str = "cli_missing";
const UPSTREAM_REQUEST_TIMEOUT_SECONDS: u64 = 600;

/// Runs the updater command-line entrypoint.
pub async fn run(cli: Cli) -> Result<()> {
    let paths = RuntimePaths::detect()?;
    paths.ensure_dirs()?;
    logging::init(&paths.log_file)?;

    let config = RuntimeConfig::load_or_default(&paths)?;
    let mut state =
        PersistedState::load_or_default(&paths.state_file, config.auto_install_on_app_exit)?;
    state.installed_version = install::installed_package_version();
    state.save(&paths.state_file)?;

    match cli.command {
        Commands::Daemon => run_daemon(&config, &mut state, &paths).await,
        Commands::CheckNow { if_stale } => {
            run_check_now(&config, &mut state, &paths, if_stale).await
        }
        Commands::CliPreflight {
            cli_path,
            print_path,
            allow_install_missing,
        } => run_cli_preflight(
            &config,
            &mut state,
            &paths,
            cli_path,
            print_path,
            allow_install_missing,
        ),
        Commands::Status { json } => run_status(&config, &mut state, &paths, json),
        Commands::InstallDeb { path } => install::install_deb(&path),
        Commands::InstallRpm { path } => install::install_rpm(&path),
        Commands::InstallPacman { path } => install::install_pacman(&path),
    }
}

fn persist_state(paths: &RuntimePaths, state: &PersistedState) -> Result<()> {
    state.save(&paths.state_file)
}

fn sync_runtime_state(config: &RuntimeConfig, state: &mut PersistedState) {
    state.auto_install_on_app_exit = config.auto_install_on_app_exit;
    state.installed_version = install::installed_package_version();
}

fn sync_and_persist(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<()> {
    sync_runtime_state(config, state);
    persist_state(paths, state)
}

fn set_status(
    state: &mut PersistedState,
    paths: &RuntimePaths,
    status: UpdateStatus,
) -> Result<()> {
    state.status = status;
    persist_state(paths, state)
}

fn mark_failed_and_persist(
    state: &mut PersistedState,
    paths: &RuntimePaths,
    message: impl Into<String>,
) -> Result<()> {
    state.mark_failed(message);
    persist_state(paths, state)
}

fn packaged_runtime_removed(config: &RuntimeConfig) -> bool {
    config.builder_bundle_root == Path::new("/usr/lib/codex-app/update-builder")
        && !config.app_executable_path.exists()
        && !install::is_primary_package_installed()
}

fn summarize_command_output(output: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(output);
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    let mut lines = text.lines().rev().take(3).collect::<Vec<_>>();
    lines.reverse();
    Some(lines.join(" | "))
}

struct CheckLock {
    _file: fs::File,
}

fn try_acquire_check_lock(paths: &RuntimePaths) -> Result<Option<CheckLock>> {
    let lock_path = paths.state_dir.join("check.lock");
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("Failed to open {}", lock_path.display()))?;

    match FileExt::try_lock(&file) {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            info!("skipping upstream check because another check is already active");
            return Ok(None);
        }
        Err(TryLockError::Error(error)) => {
            return Err(error).with_context(|| format!("Failed to lock {}", lock_path.display()));
        }
    }

    file.set_len(0)
        .with_context(|| format!("Failed to truncate {}", lock_path.display()))?;
    file.seek(SeekFrom::Start(0))
        .with_context(|| format!("Failed to seek {}", lock_path.display()))?;
    writeln!(file, "{}", std::process::id())
        .with_context(|| format!("Failed to write {}", lock_path.display()))?;

    Ok(Some(CheckLock { _file: file }))
}

fn update_install_is_pending(status: &UpdateStatus) -> bool {
    matches!(
        status,
        UpdateStatus::ReadyToInstall | UpdateStatus::WaitingForAppExit | UpdateStatus::Installing
    )
}

async fn run_daemon(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<()> {
    sync_and_persist(config, state, paths)?;
    recover_interrupted_install(state, paths)?;
    if packaged_runtime_removed(config) {
        info!("packaged app files are gone; stopping updater daemon");
        return Ok(());
    }
    codex_cli::refresh_status(config, state, paths)?;
    maybe_notify_cli_missing(state, paths, config.notifications)?;
    maybe_notify_installed(state, paths, config.notifications)?;
    info!("daemon initialized");

    time::sleep(Duration::from_secs(config.initial_check_delay_seconds)).await;
    if let Err(error) = run_check_cycle(config, state, paths).await {
        error!(?error, "initial check failed");
    }
    if let Err(error) = reconcile_pending_install(config, state, paths).await {
        error!(?error, "initial reconciliation failed");
    }

    let mut check_interval =
        time::interval(Duration::from_secs(config.check_interval_hours * 3600));
    let mut reconcile_interval = time::interval(Duration::from_secs(RECONCILE_INTERVAL_SECONDS));
    check_interval.tick().await;
    reconcile_interval.tick().await;
    loop {
        if packaged_runtime_removed(config) {
            info!("packaged app files are gone; stopping updater daemon");
            break;
        }

        tokio::select! {
            _ = check_interval.tick() => {
                if let Err(error) = run_check_cycle(config, state, paths).await {
                    error!(?error, "periodic check failed");
                }
            }
            _ = reconcile_interval.tick() => {
                if let Err(error) = reconcile_pending_install(config, state, paths).await {
                    error!(?error, "pending install reconciliation failed");
                }
            }
            signal = tokio::signal::ctrl_c() => {
                signal?;
                info!("daemon received shutdown signal");
                break;
            }
        }
    }

    Ok(())
}

async fn run_check_now(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
    if_stale: bool,
) -> Result<()> {
    sync_and_persist(config, state, paths)?;
    recover_interrupted_install(state, paths)?;
    codex_cli::refresh_status(config, state, paths)?;
    maybe_notify_cli_missing(state, paths, config.notifications)?;
    maybe_notify_installed(state, paths, config.notifications)?;
    if if_stale && state.status != UpdateStatus::Failed && upstream_check_is_fresh(config, state) {
        info!("skipping check-now because the last successful upstream check is still fresh");
        return reconcile_pending_install(config, state, paths).await;
    }
    run_check_cycle(config, state, paths).await?;
    reconcile_pending_install(config, state, paths).await
}

fn run_status(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
    json: bool,
) -> Result<()> {
    codex_cli::refresh_status(config, state, paths)?;

    if json {
        println!("{}", serde_json::to_string_pretty(state)?);
    } else {
        print!("{}", status_text(state));
    }

    Ok(())
}

fn upstream_check_is_fresh(config: &RuntimeConfig, state: &PersistedState) -> bool {
    let Some(last_successful_check_at) = state.last_successful_check_at else {
        return false;
    };

    let elapsed = Utc::now().signed_duration_since(last_successful_check_at);
    if elapsed < ChronoDuration::zero() {
        return false;
    }

    let Ok(check_interval_hours) = i64::try_from(config.check_interval_hours) else {
        return false;
    };
    let freshness_window = ChronoDuration::hours(check_interval_hours);
    elapsed < freshness_window
}

fn status_text(state: &PersistedState) -> String {
    format!(
        "\
status: {}
installed_version: {}
candidate_version: {}
cli_status: {}
cli_path: {}
cli_path_source: {}
cli_installed_version: {}
cli_latest_version: {}
cli_error: {}
",
        state.status,
        state.installed_version,
        state.candidate_version.as_deref().unwrap_or("none"),
        state.cli_status,
        state
            .cli_path
            .as_deref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        state.cli_path_source,
        state.cli_installed_version.as_deref().unwrap_or("unknown"),
        state.cli_latest_version.as_deref().unwrap_or("unknown"),
        state.cli_error_message.as_deref().unwrap_or("none")
    )
}

fn run_cli_preflight(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
    cli_path: Option<std::path::PathBuf>,
    print_path: bool,
    allow_install_missing: bool,
) -> Result<()> {
    match codex_cli::preflight(config, state, paths, cli_path, allow_install_missing) {
        Ok(outcome) => {
            if print_path {
                println!("{}", outcome.cli_path.display());
            }
            Ok(())
        }
        Err(error) => {
            if print_path {
                if let Some(path) = printable_cli_path_after_preflight_error(&error, state) {
                    println!("{}", path.display());
                }
            }
            Err(error)
        }
    }
}

fn printable_cli_path_after_preflight_error<'a>(
    error: &anyhow::Error,
    state: &'a PersistedState,
) -> Option<&'a Path> {
    if codex_cli::is_invalid_configured_cli_path_error(error) {
        return None;
    }
    if state.cli_status != CliStatus::Failed {
        return None;
    }
    state
        .cli_path
        .as_deref()
        .filter(|path| codex_cli::is_usable_cli_path(path))
}

async fn run_check_cycle(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<()> {
    let retrying_failed_update = state.status == UpdateStatus::Failed;

    if update_install_is_pending(&state.status) {
        info!("skipping upstream check because an update is already pending");
        return Ok(());
    }

    let Some(_check_lock) = try_acquire_check_lock(paths)? else {
        return Ok(());
    };

    let client = Client::builder()
        .timeout(Duration::from_secs(UPSTREAM_REQUEST_TIMEOUT_SECONDS))
        .build()?;

    sync_runtime_state(config, state);
    state.status = UpdateStatus::CheckingUpstream;
    state.last_check_at = Some(Utc::now());
    state.error_message = None;
    persist_state(paths, state)?;

    let result: Result<()> = async {
        let metadata = upstream::fetch_remote_metadata(&client, &config.dmg_url).await?;
        let previous_headers_fingerprint = state.remote_headers_fingerprint.clone();
        state.remote_headers_fingerprint = Some(metadata.headers_fingerprint.clone());
        state.last_successful_check_at = Some(Utc::now());

        if previous_headers_fingerprint.as_deref() == Some(metadata.headers_fingerprint.as_str())
            && state.dmg_sha256.is_some()
            && !retrying_failed_update
        {
            set_status(state, paths, UpdateStatus::Idle)?;
            info!("upstream fingerprint unchanged; skipping download");
            return Ok(());
        }

        set_status(state, paths, UpdateStatus::DownloadingDmg)?;

        let downloads_dir = config.workspace_root.join("downloads");
        let downloaded = upstream::download_dmg(&client, &config.dmg_url, &downloads_dir).await?;

        if state.dmg_sha256.as_deref() == Some(downloaded.sha256.as_str())
            && !retrying_failed_update
        {
            state.status = UpdateStatus::Idle;
            state.artifact_paths.dmg_path = Some(downloaded.path);
            persist_state(paths, state)?;
            info!("downloaded DMG hash matches current cached DMG; no update detected");
            return Ok(());
        }

        state.status = UpdateStatus::UpdateDetected;
        state.candidate_version = None;
        state.dmg_sha256 = Some(downloaded.sha256.clone());
        state.artifact_paths.dmg_path = Some(downloaded.path.clone());
        state.notified_events.clear();
        state.save(&paths.state_file)?;

        maybe_notify(
            state,
            paths,
            config.notifications,
            "update_detected",
            "New Codex update detected",
            "Preparing a local Linux package from the new upstream DMG.",
        )?;

        if builder::build_update(config, state, paths, &downloaded.sha256, &downloaded.path)
            .await?
            .is_some()
        {
            maybe_notify(
                state,
                paths,
                config.notifications,
                "ready_to_install",
                "Codex update ready",
                "A rebuilt Linux package is ready to install.",
            )?;
        }
        Ok(())
    }
    .await;

    if let Err(error) = result {
        mark_failed_and_persist(state, paths, error.to_string())?;
        let _ = notify_failure(config, state, paths, &error);
        return Err(error);
    }

    Ok(())
}

async fn reconcile_pending_install(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<()> {
    sync_runtime_state(config, state);
    recover_interrupted_install(state, paths)?;

    match state.status {
        UpdateStatus::ReadyToInstall | UpdateStatus::WaitingForAppExit => {
            let Some(package_path) = state.artifact_paths.package_path.clone() else {
                return Ok(());
            };

            if !package_path.exists() {
                mark_failed_and_persist(
                    state,
                    paths,
                    format!(
                        "Pending package artifact is missing: {}",
                        package_path.display()
                    ),
                )?;
                return Ok(());
            }

            if liveness::is_app_running(config)? {
                set_status(state, paths, UpdateStatus::WaitingForAppExit)?;
                maybe_notify(
                    state,
                    paths,
                    config.notifications,
                    "waiting_for_app_exit",
                    "Codex update ready",
                    "An update is ready and will install after you close Codex.",
                )?;
                return Ok(());
            }

            if !state.auto_install_on_app_exit {
                set_status(state, paths, UpdateStatus::ReadyToInstall)?;
                return Ok(());
            }

            trigger_install(state, paths, &package_path).await?;
        }
        _ => {}
    }

    Ok(())
}

fn recover_interrupted_install(state: &mut PersistedState, paths: &RuntimePaths) -> Result<()> {
    if state.status != UpdateStatus::Installing {
        return Ok(());
    }

    if state.candidate_version.as_deref().is_some_and(|candidate| {
        package_version::installed_version_satisfies_candidate(&state.installed_version, candidate)
    }) {
        state.status = UpdateStatus::Installed;
        state.candidate_version = None;
        state.error_message = None;
        state.notified_events.clear();
        persist_state(paths, state)?;
        info!("recovered interrupted install state because the candidate version is already installed");
        return Ok(());
    }

    let Some(package_path) = state.artifact_paths.package_path.clone() else {
        mark_failed_and_persist(
            state,
            paths,
            "Previous install attempt was interrupted and no package artifact is recorded",
        )?;
        return Ok(());
    };

    if !package_path.exists() {
        mark_failed_and_persist(
            state,
            paths,
            format!(
                "Previous install attempt was interrupted and the package artifact is missing: {}",
                package_path.display()
            ),
        )?;
        return Ok(());
    }

    state.status = UpdateStatus::ReadyToInstall;
    state.error_message =
        Some("Previous install attempt was interrupted before completion".to_string());
    persist_state(paths, state)?;
    info!(package = %package_path.display(), "recovered interrupted install state back to ready_to_install");
    Ok(())
}

fn maybe_notify(
    state: &mut PersistedState,
    paths: &RuntimePaths,
    enabled: bool,
    event_name: &str,
    summary: &str,
    body: &str,
) -> Result<()> {
    let version = state
        .candidate_version
        .as_deref()
        .unwrap_or(&state.installed_version);
    let event_key = format!("{event_name}:{version}");
    maybe_notify_with_event_key(state, paths, enabled, &event_key, summary, body)
}

fn maybe_notify_with_event_key(
    state: &mut PersistedState,
    paths: &RuntimePaths,
    enabled: bool,
    event_key: &str,
    summary: &str,
    body: &str,
) -> Result<()> {
    if !state.notified_events.insert(event_key.to_string()) {
        return Ok(());
    }

    if enabled {
        if let Err(error) = notify::send(summary, body) {
            warn!(?error, "failed to send desktop notification");
        }
    }

    persist_state(paths, state)?;
    Ok(())
}

fn clear_notification_event(
    state: &mut PersistedState,
    paths: &RuntimePaths,
    event_key: &str,
) -> Result<()> {
    if state.notified_events.remove(event_key) {
        persist_state(paths, state)?;
    }

    Ok(())
}

fn cli_is_missing(state: &PersistedState) -> bool {
    state.cli_path.is_none() && state.cli_installed_version.is_none()
}

fn maybe_notify_cli_missing(
    state: &mut PersistedState,
    paths: &RuntimePaths,
    enabled: bool,
) -> Result<()> {
    if !cli_is_missing(state) {
        return clear_notification_event(state, paths, CLI_MISSING_NOTIFICATION_EVENT);
    }

    maybe_notify_with_event_key(
        state,
        paths,
        enabled,
        CLI_MISSING_NOTIFICATION_EVENT,
        "Codex CLI not installed",
        "Codex needs the Codex CLI. Install it with npm or open the app to retry the automatic install flow.",
    )
}

fn maybe_notify_installed(
    state: &mut PersistedState,
    paths: &RuntimePaths,
    enabled: bool,
) -> Result<()> {
    if state.status != UpdateStatus::Installed {
        return Ok(());
    }

    maybe_notify(
        state,
        paths,
        enabled,
        "installed",
        "Codex updated",
        "The new package is installed and will be used the next time you open the app.",
    )
}

async fn trigger_install(
    state: &mut PersistedState,
    paths: &RuntimePaths,
    package_path: &Path,
) -> Result<()> {
    state.status = UpdateStatus::Installing;
    state.error_message = None;
    persist_state(paths, state)?;

    let _ = notify::send(
        "Installing Codex update",
        "Applying the locally rebuilt Linux package.",
    );

    let current_exe = std::env::current_exe().context("Failed to resolve updater binary path")?;
    let output = install::pkexec_command(&current_exe, package_path)
        .output()
        .context("Failed to launch pkexec for update installation")?;
    let status = output.status;

    if status.success() {
        state.status = UpdateStatus::Installed;
        state.installed_version = install::installed_package_version();
        state.candidate_version = None;
        state.error_message = None;
        state.notified_events.clear();
        persist_state(paths, state)?;
        let _ = maybe_notify_installed(state, paths, true);
        return Ok(());
    }

    let stdout = summarize_command_output(&output.stdout);
    let stderr = summarize_command_output(&output.stderr);
    error!(
        status = %status,
        stdout = stdout.as_deref().unwrap_or(""),
        stderr = stderr.as_deref().unwrap_or(""),
        "privileged install failed"
    );

    let mut message = format!("Privileged install exited with status {status}");
    if let Some(stderr) = stderr {
        message.push_str(": ");
        message.push_str(&stderr);
    }

    let error = anyhow::anyhow!(message);
    mark_failed_and_persist(state, paths, error.to_string())?;
    let _ = notify::send(
        "Codex update failed",
        "The package could not be installed. Check the updater log for details.",
    );
    Err(error)
}

fn notify_failure(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
    error: &anyhow::Error,
) -> Result<()> {
    let body = format!("The local rebuild failed: {error}");
    maybe_notify(
        state,
        paths,
        config.notifications,
        "build_failed",
        "Codex update failed",
        &body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::CliPathSource;

    #[test]
    fn upstream_check_freshness_respects_configured_interval() {
        let config = RuntimeConfig {
            dmg_url: "https://example.com/Codex.dmg".to_string(),
            initial_check_delay_seconds: 1,
            check_interval_hours: 6,
            auto_install_on_app_exit: true,
            notifications: false,
            developer_mode: false,
            workspace_root: std::path::PathBuf::from("/tmp/cache"),
            builder_bundle_root: std::path::PathBuf::from("/tmp/builder"),
            app_executable_path: std::path::PathBuf::from("/tmp/electron"),
            cli_path: None,
        };

        let mut state = PersistedState::new(true);
        assert!(!upstream_check_is_fresh(&config, &state));

        state.last_successful_check_at = Some(Utc::now() - ChronoDuration::hours(1));
        assert!(upstream_check_is_fresh(&config, &state));

        state.last_successful_check_at = Some(Utc::now() - ChronoDuration::hours(7));
        assert!(!upstream_check_is_fresh(&config, &state));

        state.last_successful_check_at = Some(Utc::now() + ChronoDuration::hours(1));
        assert!(!upstream_check_is_fresh(&config, &state));
    }

    #[test]
    fn upstream_check_freshness_rejects_out_of_range_interval() {
        let config = RuntimeConfig {
            dmg_url: "https://example.com/Codex.dmg".to_string(),
            initial_check_delay_seconds: 1,
            check_interval_hours: u64::MAX,
            auto_install_on_app_exit: true,
            notifications: false,
            developer_mode: false,
            workspace_root: std::path::PathBuf::from("/tmp/cache"),
            builder_bundle_root: std::path::PathBuf::from("/tmp/builder"),
            app_executable_path: std::path::PathBuf::from("/tmp/electron"),
            cli_path: None,
        };
        let mut state = PersistedState::new(true);
        state.last_successful_check_at = Some(Utc::now() - ChronoDuration::hours(1));

        assert!(!upstream_check_is_fresh(&config, &state));
    }

    #[tokio::test]
    async fn failed_state_with_existing_deb_stays_failed() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        paths.ensure_dirs()?;

        let package_path = temp.path().join("dist/codex.deb");
        std::fs::create_dir_all(
            package_path
                .parent()
                .expect("package path should have parent"),
        )?;
        std::fs::write(&package_path, b"deb")?;

        let config = RuntimeConfig {
            dmg_url: "https://example.com/Codex.dmg".to_string(),
            initial_check_delay_seconds: 1,
            check_interval_hours: 6,
            auto_install_on_app_exit: false,
            notifications: false,
            developer_mode: false,
            workspace_root: temp.path().join("cache"),
            builder_bundle_root: temp.path().join("builder"),
            app_executable_path: temp.path().join("not-running-electron"),
            cli_path: None,
        };

        let mut state = PersistedState::new(false);
        state.status = UpdateStatus::Failed;
        state.candidate_version = Some("26.422.30944.2080".to_string());
        state.error_message = Some("previous failure".to_string());
        state.artifact_paths.package_path = Some(package_path);

        reconcile_pending_install(&config, &mut state, &paths).await?;

        assert_eq!(state.status, UpdateStatus::Failed);
        assert_eq!(state.error_message.as_deref(), Some("previous failure"));
        Ok(())
    }

    #[test]
    fn status_text_includes_cli_path_and_source() {
        let mut state = PersistedState::new(true);
        state.cli_status = CliStatus::UpToDate;
        state.cli_path = Some(Path::new("/home/user/.local/bin/codex").to_path_buf());
        state.cli_path_source = CliPathSource::KnownPath;
        state.cli_installed_version = Some("0.42.0".to_string());
        state.cli_latest_version = Some("0.42.0".to_string());

        let output = status_text(&state);

        assert!(output.contains("cli_path: /home/user/.local/bin/codex"));
        assert!(output.contains("cli_status: up_to_date"));
        assert!(output.contains("cli_path_source: known_path"));
    }

    #[test]
    fn failed_preflight_can_print_persisted_cli_path_for_nonconfigured_errors() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let cli_path = temp.path().join("codex");
        write_executable_script(&cli_path, "#!/bin/sh\necho 'codex-cli v0.42.0'\n")?;

        let mut state = PersistedState::new(true);
        state.cli_path = Some(cli_path.clone());
        state.cli_status = CliStatus::Failed;
        state.cli_error_message = Some("Codex CLI upgrade failed".to_string());
        let error = anyhow::anyhow!("Codex CLI upgrade failed: npm install failed");

        assert_eq!(
            printable_cli_path_after_preflight_error(&error, &state),
            Some(cli_path.as_path())
        );
        Ok(())
    }

    #[test]
    fn failed_preflight_does_not_print_path_for_invalid_configured_errors() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        paths.ensure_dirs()?;

        let cli_path = temp.path().join("codex");
        write_executable_script(&cli_path, "#!/bin/sh\necho 'codex-cli v0.42.0'\n")?;
        let mut state = PersistedState::new(true);
        state.cli_path = Some(cli_path);

        let mut config = RuntimeConfig::default_with_paths(&paths);
        config.cli_path = Some(temp.path().join("missing-codex"));

        let error = codex_cli::preflight(&config, &mut state, &paths, None, false)
            .expect_err("invalid configured path should fail loudly");

        assert!(codex_cli::is_invalid_configured_cli_path_error(&error));
        assert_eq!(
            printable_cli_path_after_preflight_error(&error, &state),
            None
        );
        Ok(())
    }

    fn write_executable_script(path: &Path, contents: &str) -> Result<()> {
        std::fs::write(path, contents)?;
        let mut permissions = std::fs::metadata(path)?.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(0o755);
        }
        std::fs::set_permissions(path, permissions)?;
        Ok(())
    }

    #[tokio::test]
    async fn run_check_cycle_skips_when_update_is_already_pending() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        paths.ensure_dirs()?;

        let config = RuntimeConfig {
            dmg_url: "https://invalid.example/Codex.dmg".to_string(),
            initial_check_delay_seconds: 1,
            check_interval_hours: 6,
            auto_install_on_app_exit: true,
            notifications: false,
            developer_mode: false,
            workspace_root: temp.path().join("cache"),
            builder_bundle_root: temp.path().join("builder"),
            app_executable_path: temp.path().join("not-running-electron"),
            cli_path: None,
        };

        for status in [
            UpdateStatus::ReadyToInstall,
            UpdateStatus::WaitingForAppExit,
            UpdateStatus::Installing,
        ] {
            let mut state = PersistedState::new(true);
            state.status = status.clone();

            run_check_cycle(&config, &mut state, &paths).await?;

            assert_eq!(state.status, status);
            assert_eq!(state.last_check_at, None);
        }

        Ok(())
    }

    #[test]
    fn check_lock_file_without_kernel_lock_does_not_block_acquire() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        paths.ensure_dirs()?;
        let lock_path = paths.state_dir.join("check.lock");
        std::fs::write(&lock_path, b"stale-pid")?;

        let lock = try_acquire_check_lock(&paths)?;

        assert!(lock.is_some());
        assert_eq!(
            std::fs::read_to_string(&lock_path)?.trim(),
            std::process::id().to_string()
        );
        Ok(())
    }

    #[test]
    fn held_check_lock_blocks_second_acquire() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        paths.ensure_dirs()?;

        let first_lock = try_acquire_check_lock(&paths)?;
        let second_lock = try_acquire_check_lock(&paths)?;

        assert!(first_lock.is_some());
        assert!(second_lock.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn missing_pending_package_marks_state_failed() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        paths.ensure_dirs()?;

        let config = RuntimeConfig {
            dmg_url: "https://example.com/Codex.dmg".to_string(),
            initial_check_delay_seconds: 1,
            check_interval_hours: 6,
            auto_install_on_app_exit: true,
            notifications: false,
            developer_mode: false,
            workspace_root: temp.path().join("cache"),
            builder_bundle_root: temp.path().join("builder"),
            app_executable_path: temp.path().join("not-running-electron"),
            cli_path: None,
        };

        let mut state = PersistedState::new(true);
        state.status = UpdateStatus::ReadyToInstall;
        state.candidate_version = Some("26.422.30944.2080".to_string());
        state.artifact_paths.package_path = Some(temp.path().join("missing/codex.deb"));

        reconcile_pending_install(&config, &mut state, &paths).await?;

        assert_eq!(state.status, UpdateStatus::Failed);
        assert!(state
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains("Pending package artifact is missing")));
        Ok(())
    }

    #[tokio::test]
    async fn ready_update_respects_manual_install_mode() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        paths.ensure_dirs()?;

        let package_path = temp.path().join("dist/codex.deb");
        std::fs::create_dir_all(
            package_path
                .parent()
                .expect("package path should have parent"),
        )?;
        std::fs::write(&package_path, b"deb")?;

        let config = RuntimeConfig {
            dmg_url: "https://example.com/Codex.dmg".to_string(),
            initial_check_delay_seconds: 1,
            check_interval_hours: 6,
            auto_install_on_app_exit: false,
            notifications: false,
            developer_mode: false,
            workspace_root: temp.path().join("cache"),
            builder_bundle_root: temp.path().join("builder"),
            app_executable_path: temp.path().join("not-running-electron"),
            cli_path: None,
        };

        let mut state = PersistedState::new(false);
        state.status = UpdateStatus::ReadyToInstall;
        state.candidate_version = Some("26.422.30944.2080".to_string());
        state.artifact_paths.package_path = Some(package_path);

        reconcile_pending_install(&config, &mut state, &paths).await?;

        assert_eq!(state.status, UpdateStatus::ReadyToInstall);
        assert_eq!(state.error_message, None);
        Ok(())
    }

    #[tokio::test]
    async fn interrupted_install_becomes_installed_when_candidate_is_already_present() -> Result<()>
    {
        let temp = tempfile::tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        paths.ensure_dirs()?;

        let package_path = temp.path().join("dist/codex.deb");
        std::fs::create_dir_all(
            package_path
                .parent()
                .expect("package path should have parent"),
        )?;
        std::fs::write(&package_path, b"deb")?;

        let mut state = PersistedState::new(true);
        state.status = UpdateStatus::Installing;
        state.installed_version = "26.422.30944.2080".to_string();
        state.candidate_version = Some("26.422.30944.2079".to_string());
        state.artifact_paths.package_path = Some(package_path);

        recover_interrupted_install(&mut state, &paths)?;

        assert_eq!(state.status, UpdateStatus::Installed);
        assert_eq!(state.candidate_version, None);
        assert_eq!(state.error_message, None);
        Ok(())
    }

    #[tokio::test]
    async fn interrupted_install_returns_to_ready_when_package_still_exists() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        paths.ensure_dirs()?;

        let package_path = temp.path().join("dist/codex.deb");
        std::fs::create_dir_all(
            package_path
                .parent()
                .expect("package path should have parent"),
        )?;
        std::fs::write(&package_path, b"deb")?;

        let mut state = PersistedState::new(true);
        state.status = UpdateStatus::Installing;
        state.installed_version = "26.422.30944.2079".to_string();
        state.candidate_version = Some("26.422.30944.2080".to_string());
        state.artifact_paths.package_path = Some(package_path);

        recover_interrupted_install(&mut state, &paths)?;

        assert_eq!(state.status, UpdateStatus::ReadyToInstall);
        assert!(state
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains("interrupted")));
        Ok(())
    }

    #[test]
    fn notification_events_are_deduplicated() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        paths.ensure_dirs()?;

        let mut state = PersistedState::new(true);
        state.candidate_version = Some("26.422.30944.2080".to_string());
        maybe_notify(
            &mut state,
            &paths,
            false,
            "ready_to_install",
            "Codex update ready",
            "An update is ready to install.",
        )?;
        let notified_count = state.notified_events.len();
        maybe_notify(
            &mut state,
            &paths,
            false,
            "ready_to_install",
            "Codex update ready",
            "An update is ready to install.",
        )?;

        assert_eq!(state.notified_events.len(), notified_count);
        Ok(())
    }

    #[test]
    fn installed_notifications_are_deduplicated_after_recovery() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        paths.ensure_dirs()?;

        let mut state = PersistedState::new(true);
        state.status = UpdateStatus::Installed;
        state.installed_version = "26.422.30944.2080".to_string();

        maybe_notify_installed(&mut state, &paths, false)?;
        let notified_count = state.notified_events.len();
        maybe_notify_installed(&mut state, &paths, false)?;

        assert_eq!(state.notified_events.len(), notified_count);
        assert!(state
            .notified_events
            .contains("installed:26.422.30944.2080"));
        Ok(())
    }

    #[test]
    fn cli_missing_notifications_are_deduplicated() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        paths.ensure_dirs()?;

        let mut state = PersistedState::new(true);
        state.cli_path = None;
        state.cli_installed_version = None;
        state.cli_error_message = Some("Codex CLI not found in configured paths".to_string());

        maybe_notify_cli_missing(&mut state, &paths, false)?;
        let notified_count = state.notified_events.len();
        maybe_notify_cli_missing(&mut state, &paths, false)?;

        assert_eq!(state.notified_events.len(), notified_count);
        assert!(state.notified_events.contains("cli_missing"));
        Ok(())
    }

    #[test]
    fn cli_missing_notification_marker_is_cleared_after_recovery() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        paths.ensure_dirs()?;

        let mut state = PersistedState::new(true);
        state.notified_events.insert("cli_missing".to_string());
        state.cli_path = Some(temp.path().join("codex"));
        state.cli_installed_version = Some("0.42.0".to_string());
        state.cli_error_message = None;

        maybe_notify_cli_missing(&mut state, &paths, false)?;

        assert!(!state.notified_events.contains("cli_missing"));
        Ok(())
    }

}
