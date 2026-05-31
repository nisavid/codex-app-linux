//! Application entrypoints and orchestration for the local updater daemon.

use crate::{
    builder, cache_cleanup,
    cli::{Cli, Commands},
    codex_cli,
    config::{RuntimeConfig, RuntimePaths},
    dmg_source, install, install_rollback, liveness, logging, notify, package_verification,
    redaction, rollback, feature_picker,
    state::{CliStatus, DmgVerification, DmgVerificationResult, PersistedState, UpdateStatus},
    trust, wrapper, wrapper_apply,
};
use anyhow::{Context, Result};
use chrono::{Duration as ChronoDuration, Utc};
use fs4::{FileExt, TryLockError};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{Seek, SeekFrom, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};
use tokio::io::AsyncReadExt;
use tokio::time::{self, Duration};
use tracing::{error, info, warn};

const RECONCILE_INTERVAL_SECONDS: u64 = 15;
const CLI_MISSING_NOTIFICATION_EVENT: &str = "cli_missing";
const CLI_MISSING_PROMPT_DISMISS_TTL: ChronoDuration = ChronoDuration::minutes(10);
const PROMPT_INSTALL_CLI_CANCELLED_EXIT_CODE: i32 = 10;
const PROMPT_INSTALL_CLI_NO_BACKEND_EXIT_CODE: i32 = 11;
const UPSTREAM_REQUEST_TIMEOUT_SECONDS: u64 = 600;

/// Runs the updater command-line entrypoint.
pub async fn run(cli: Cli) -> Result<()> {
    let paths = RuntimePaths::detect()?;
    paths.ensure_dirs()?;
    logging::init(&paths.log_file)?;

    let mut config = RuntimeConfig::load_or_default(&paths)?;
    if let Some(enabled) = crate::config::settings_wrapper_updates_override() {
        config.enable_wrapper_updates = enabled;
    }
    let mut state =
        PersistedState::load_or_default(&paths.state_file, effective_auto_install(&config))?;
    let original_state = state.clone();
    state.installed_version = install::installed_package_version();
    persist_if_changed(&paths, &state, &original_state)?;

    match cli.command {
        Commands::Daemon => run_daemon(&config, &mut state, &paths).await,
        Commands::CheckNow { if_stale } => {
            run_check_now(&config, &mut state, &paths, if_stale).await
        }
        Commands::CheckWrapper { json } => run_check_wrapper(&config, &mut state, &paths, json),
        Commands::ApplyWrapperUpdate => {
            wrapper_apply::run_apply_wrapper_update(&config, &mut state, &paths).await
        }
        Commands::PickFeatures { json } => {
            feature_picker::run_pick_integrations(&config, &paths, json)
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
        Commands::PromptInstallCli {
            cli_path,
            print_path,
        } => run_prompt_install_cli(&config, &mut state, &paths, cli_path, print_path),
        Commands::Status { json } => run_status(&config, &mut state, &paths, json),
        Commands::InstallReady => run_install_ready(&config, &mut state, &paths).await,
        Commands::Rollback => rollback::run(&config, &mut state, &paths).await,
        Commands::InstallDeb {
            path,
            expected_sha256,
            expected_package_name,
            expected_package_version,
        } => {
            let expected = install::expected_package_from_args(
                expected_sha256,
                expected_package_name,
                expected_package_version,
            )?;
            install::install_deb(&path, expected.as_ref())
        }
        Commands::InstallRpm {
            path,
            expected_sha256,
            expected_package_name,
            expected_package_version,
        } => {
            let expected = install::expected_package_from_args(
                expected_sha256,
                expected_package_name,
                expected_package_version,
            )?;
            install::install_rpm(&path, expected.as_ref())
        }
        Commands::InstallPacman {
            path,
            expected_sha256,
            expected_package_name,
            expected_package_version,
        } => {
            let expected = install::expected_package_from_args(
                expected_sha256,
                expected_package_name,
                expected_package_version,
            )?;
            install::install_pacman(&path, expected.as_ref())
        }
        Commands::InstallRollbackDeb {
            path,
            expected_sha256,
            expected_package_name,
            expected_package_version,
        } => {
            let expected = install::expected_package_from_args(
                expected_sha256,
                expected_package_name,
                expected_package_version,
            )?;
            install_rollback::install_deb(&path, expected.as_ref())
        }
        Commands::InstallRollbackRpm {
            path,
            expected_sha256,
            expected_package_name,
            expected_package_version,
        } => {
            let expected = install::expected_package_from_args(
                expected_sha256,
                expected_package_name,
                expected_package_version,
            )?;
            install_rollback::install_rpm(&path, expected.as_ref())
        }
        Commands::InstallRollbackPacman {
            path,
            expected_sha256,
            expected_package_name,
            expected_package_version,
        } => {
            let expected = install::expected_package_from_args(
                expected_sha256,
                expected_package_name,
                expected_package_version,
            )?;
            install_rollback::install_pacman(&path, expected.as_ref())
        }
    }
}

fn persist_state(paths: &RuntimePaths, state: &PersistedState) -> Result<()> {
    state.save(&paths.state_file)
}

fn persist_if_changed(
    paths: &RuntimePaths,
    state: &PersistedState,
    original_state: &PersistedState,
) -> Result<()> {
    if state != original_state {
        persist_state(paths, state)?;
    }

    Ok(())
}

fn effective_auto_install(config: &RuntimeConfig) -> bool {
    crate::config::settings_auto_install_override().unwrap_or(config.auto_install_on_app_exit)
}

fn sync_runtime_state(config: &RuntimeConfig, state: &mut PersistedState) {
    state.auto_install_on_app_exit = effective_auto_install(config);
    if state.status != UpdateStatus::WaitingForAppExit {
        state.waiting_for_app_exit_auto_install = false;
    }
    state.installed_version = install::installed_package_version();
}

fn sync_and_persist(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<()> {
    let original_state = state.clone();
    sync_runtime_state(config, state);
    persist_if_changed(paths, state, &original_state)
}

fn normalize_workspace_dir_and_persist(
    workspace_root: &Path,
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<()> {
    let original_state = state.clone();
    cache_cleanup::normalize_artifact_workspace_dir(workspace_root, state);
    persist_if_changed(paths, state, &original_state)
}

fn maybe_prune_workspace_cache(workspace_root: &Path, state: &PersistedState) {
    match cache_cleanup::prune_unreferenced_workspaces(workspace_root, state) {
        Ok(summary) if summary.pruned_workspaces > 0 => {
            info!(
                pruned_workspaces = summary.pruned_workspaces,
                workspace_root = %workspace_root.display(),
                "pruned unreferenced updater workspaces"
            );
        }
        Ok(_) => {}
        Err(error) => {
            warn!(
                ?error,
                workspace_root = %workspace_root.display(),
                "failed to prune unreferenced updater workspaces"
            );
        }
    }
}

fn clear_wrapper_update_candidate_and_persist(
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<()> {
    let original_state = state.clone();
    state.clear_wrapper_update_candidate();
    persist_if_changed(paths, state, &original_state)
}

fn refresh_installed_wrapper_state(config: &RuntimeConfig, state: &mut PersistedState) {
    if let Some(installed) = wrapper::installed_wrapper_from_metadata(
        &config.app_executable_path,
        &config.builder_bundle_root,
    ) {
        state.installed_wrapper_version = installed.version;
        state.installed_wrapper_commit = Some(installed.commit);
    } else {
        state.installed_wrapper_version = None;
        state.installed_wrapper_commit = None;
    }
}

fn clear_stale_wrapper_update_and_persist(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<()> {
    let original_state = state.clone();
    refresh_installed_wrapper_state(config, state);
    state.clear_wrapper_update_candidate();
    persist_if_changed(paths, state, &original_state)
}

fn set_status(
    state: &mut PersistedState,
    paths: &RuntimePaths,
    status: UpdateStatus,
) -> Result<()> {
    state.status = status;
    if state.status != UpdateStatus::WaitingForAppExit {
        state.waiting_for_app_exit_auto_install = false;
    }
    persist_state(paths, state)
}

fn set_waiting_for_app_exit(
    state: &mut PersistedState,
    paths: &RuntimePaths,
    auto_install: bool,
) -> Result<()> {
    state.waiting_for_app_exit_auto_install = auto_install;
    state.status = UpdateStatus::WaitingForAppExit;
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

fn record_verified_dmg(state: &mut PersistedState, verified: &trust::VerifiedDmg) {
    state.dmg_verification = Some(DmgVerification {
        result: DmgVerificationResult::Verified,
        version: Some(verified.version.clone()),
        sha256: Some(verified.sha256.clone()),
        manifest_path: Some(verified.manifest_path.clone()),
        verified_at: Some(Utc::now()),
        message: Some("Downloaded DMG matched repo-trusted metadata".to_string()),
    });
}

fn record_failed_dmg_verification(
    state: &mut PersistedState,
    downloaded_sha256: &str,
    manifest_path: PathBuf,
    message: String,
) {
    state.dmg_verification = Some(DmgVerification {
        result: DmgVerificationResult::Failed,
        version: None,
        sha256: Some(downloaded_sha256.to_string()),
        manifest_path: Some(manifest_path),
        verified_at: Some(Utc::now()),
        message: Some(message),
    });
}

fn ensure_ready_update_has_verified_dmg(state: &PersistedState) -> Result<()> {
    let Some(verification) = state.dmg_verification.as_ref() else {
        anyhow::bail!("Ready update is missing trusted DMG verification");
    };
    if verification.result != DmgVerificationResult::Verified {
        anyhow::bail!("Ready update is not backed by successful trusted DMG verification");
    }
    let Some(verified_version) = verification.version.as_deref() else {
        anyhow::bail!("Ready update trusted DMG verification is missing a version");
    };
    let Some(candidate_version) = state.candidate_version.as_deref() else {
        anyhow::bail!("Ready update is missing a candidate version");
    };
    if verified_version != candidate_version {
        anyhow::bail!("Ready update version does not match trusted DMG verification");
    }
    let Some(verified_sha256) = verification.sha256.as_deref() else {
        anyhow::bail!("Ready update trusted DMG verification is missing a digest");
    };
    let Some(dmg_sha256) = state.dmg_sha256.as_deref() else {
        anyhow::bail!("Ready update is missing a DMG digest");
    };
    if verified_sha256 != dmg_sha256 {
        anyhow::bail!("Ready update digest does not match trusted DMG verification");
    }
    Ok(())
}

fn state_has_verified_current_dmg(state: &PersistedState) -> bool {
    let Some(verification) = state.dmg_verification.as_ref() else {
        return false;
    };
    verification.result == DmgVerificationResult::Verified
        && verification.version.is_some()
        && state
            .candidate_version
            .as_deref()
            .is_none_or(|candidate| verification.version.as_deref() == Some(candidate))
        && verification.sha256.as_deref() == state.dmg_sha256.as_deref()
}

async fn file_sha256(path: &Path) -> Result<String> {
    let mut file = tokio::fs::File::open(path).await.with_context(|| {
        format!(
            "Failed to open {} for trusted DMG hash check",
            path.display()
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let bytes_read = file.read(&mut buffer).await.with_context(|| {
            format!(
                "Failed to read {} for trusted DMG hash check",
                path.display()
            )
        })?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>())
}

async fn ensure_downloaded_dmg_still_matches_verified_metadata(
    path: &Path,
    verified: &trust::VerifiedDmg,
) -> Result<()> {
    let current_sha256 = file_sha256(path).await?;
    if current_sha256 != verified.sha256 {
        anyhow::bail!(
            "Downloaded DMG changed after trusted metadata verification: expected {}, found {}",
            verified.sha256,
            current_sha256
        );
    }
    Ok(())
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

    let mut lines = text
        .lines()
        .rev()
        .take(3)
        .map(redaction::redact_for_persistence)
        .collect::<Vec<_>>();
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
            info!("skipping remote DMG check because another check is already active");
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

fn reconcile_cli_if_present_best_effort(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
    context: &'static str,
) {
    if let Err(error) = codex_cli::reconcile_if_present(config, state, paths) {
        warn!(?error, context, "unable to reconcile Codex CLI");
    }
}

async fn run_daemon(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<()> {
    sync_and_persist(config, state, paths)?;
    recover_interrupted_install(&config.workspace_root, state, paths)?;
    reconcile_cli_if_present_best_effort(config, state, paths, "daemon startup");
    maybe_notify_cli_missing(state, paths, config.notifications)?;
    maybe_notify_installed(state, paths, config.notifications)?;
    if packaged_runtime_removed(config) {
        info!("packaged app files are gone; stopping updater daemon");
        return Ok(());
    }
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
    recover_interrupted_install(&config.workspace_root, state, paths)?;
    reconcile_cli_if_present_best_effort(config, state, paths, "check-now");
    maybe_notify_cli_missing(state, paths, config.notifications)?;
    maybe_notify_installed(state, paths, config.notifications)?;
    if if_stale && state.status != UpdateStatus::Failed && remote_dmg_check_is_fresh(config, state)
    {
        if let Err(error) = detect_and_record_wrapper_update(config, state, paths) {
            warn!(
                ?error,
                "wrapper update detection failed during fresh check-now"
            );
        }
        info!("skipping check-now because the last successful remote DMG check is still fresh");
        return reconcile_pending_install(config, state, paths).await;
    }
    run_check_cycle(config, state, paths).await?;
    reconcile_pending_install(config, state, paths).await
}

fn remote_dmg_check_is_fresh(config: &RuntimeConfig, state: &PersistedState) -> bool {
    let Some(last_successful_check_at) = state.last_successful_check_at else {
        return false;
    };

    let elapsed = Utc::now().signed_duration_since(last_successful_check_at);
    if elapsed < ChronoDuration::zero() {
        return false;
    }

    let freshness_window = ChronoDuration::hours(config.check_interval_hours as i64);
    elapsed < freshness_window
}

/// Detects a newer wrapper release and records it into state. Returns
/// `Ok(true)` when an update was found and recorded. No-ops (returning
/// `Ok(false)`) when wrapper tracking is disabled, the builder bundle is not a
/// git checkout, or no newer commit is available. Never mutates the checkout.
fn detect_and_record_wrapper_update(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<bool> {
    if !config.enable_wrapper_updates {
        clear_wrapper_update_candidate_and_persist(state, paths)?;
        return Ok(false);
    }

    let Some(installed) = wrapper::installed_wrapper_from_metadata(
        &config.app_executable_path,
        &config.builder_bundle_root,
    ) else {
        clear_stale_wrapper_update_and_persist(config, state, paths)?;
        return Ok(false);
    };

    use wrapper::WrapperDetectionState::*;

    let detection = match wrapper::detect_state_from_bundle_root(
        &config.builder_bundle_root,
        &installed,
        &config.wrapper_remote,
        &config.wrapper_branch,
    ) {
        Ok(result) => result,
        Err(error) => {
            warn!(?error, "wrapper update detection failed");
            let original_state = state.clone();
            state.installed_wrapper_version = installed.version;
            state.installed_wrapper_commit = Some(installed.commit);
            persist_if_changed(paths, state, &original_state)?;
            return Ok(false);
        }
    };

    let original_state = state.clone();
    state.installed_wrapper_version = installed.version.clone();
    state.installed_wrapper_commit = Some(installed.commit.clone());

    match detection {
        (UpdateAvailable, Some(update)) => {
            state.wrapper_dev_mode = Some(false);
            state.installed_wrapper_version = update.installed_version.clone();
            state.installed_wrapper_commit = Some(update.installed_commit.clone());
            state.candidate_wrapper_version = update.candidate_version.clone();
            state.candidate_wrapper_commit = Some(update.candidate_commit.clone());
            state.wrapper_changelog = Some(update.changelog.clone());
            persist_if_changed(paths, state, &original_state)?;

            let change_count = update
                .changelog
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count();
            maybe_notify(
                state,
                paths,
                config.notifications,
                &format!("wrapper_update:{}", update.candidate_commit),
                "Codex App update available",
                &format!(
                    "A newer codex-app build is available ({change_count} change(s)). Rebuild to apply."
                ),
            )?;

            Ok(true)
        }
        (DevMode, _) => {
            state.clear_wrapper_update_candidate();
            state.wrapper_dev_mode = Some(true);
            persist_if_changed(paths, state, &original_state)?;
            Ok(false)
        }
        (Aligned, _) => {
            state.clear_wrapper_update_candidate();
            state.wrapper_dev_mode = Some(false);
            persist_if_changed(paths, state, &original_state)?;
            Ok(false)
        }
        (UnknownOffline, _) | (UpdateAvailable, None) => {
            state.clear_wrapper_update_candidate();
            persist_if_changed(paths, state, &original_state)?;
            Ok(false)
        }
    }
}

fn run_check_wrapper(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
    json: bool,
) -> Result<()> {
    if !config.enable_wrapper_updates {
        clear_wrapper_update_candidate_and_persist(state, paths)?;
        if json {
            println!("{}", serde_json::json!({ "enabled": false }));
        } else {
            println!(
                "Wrapper update tracking is disabled (set enable_wrapper_updates = true in config.toml)."
            );
        }
        return Ok(());
    }

    let found = detect_and_record_wrapper_update(config, state, paths)?;

    if json {
        println!("{}", serde_json::to_string_pretty(state)?);
    } else if found {
        println!(
            "wrapper update available: {} -> {}",
            state
                .installed_wrapper_commit
                .as_deref()
                .unwrap_or("unknown"),
            state
                .candidate_wrapper_commit
                .as_deref()
                .unwrap_or("unknown")
        );
        if let Some(changelog) = state.wrapper_changelog.as_deref() {
            println!("\n{changelog}");
        }
    } else if state.wrapper_dev_mode == Some(true) {
        println!("wrapper is a local/dev build ahead of upstream; updates are disabled.");
    } else {
        println!("wrapper is up to date (or not a git checkout).");
    }

    Ok(())
}

fn run_status(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
    json: bool,
) -> Result<()> {
    codex_cli::refresh_status(config, state, paths)?;
    complete_pending_install_if_already_installed(&config.workspace_root, state, paths)?;
    recover_interrupted_install(&config.workspace_root, state, paths)?;
    normalize_workspace_dir_and_persist(&config.workspace_root, state, paths)?;
    if !config.enable_wrapper_updates {
        clear_wrapper_update_candidate_and_persist(state, paths)?;
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&state.redacted_for_persistence())?
        );
    } else {
        println!("status: {:?}", state.status);
        println!("installed_version: {}", state.installed_version);
        println!(
            "candidate_version: {}",
            state.candidate_version.as_deref().unwrap_or("none")
        );
        println!(
            "last_known_good_version: {}",
            state.last_known_good_version.as_deref().unwrap_or("none")
        );
        println!(
            "rollback_blocked_candidate_version: {}",
            state
                .rollback_blocked_candidate_version
                .as_deref()
                .unwrap_or("none")
        );
        println!(
            "rollback_blocked_dmg_sha256: {}",
            state
                .rollback_blocked_dmg_sha256
                .as_deref()
                .unwrap_or("none")
        );
        println!("{}", update_error_status_line(state));
        println!("cli_status: {:?}", state.cli_status);
        println!(
            "cli_installed_version: {}",
            state.cli_installed_version.as_deref().unwrap_or("unknown")
        );
        println!(
            "cli_latest_version: {}",
            state.cli_latest_version.as_deref().unwrap_or("unknown")
        );
        println!(
            "cli_error: {}",
            state
                .cli_error_message
                .as_deref()
                .map(redaction::redact_for_persistence)
                .as_deref()
                .unwrap_or("none")
        );
    }

    Ok(())
}

fn update_error_status_line(state: &PersistedState) -> String {
    format!(
        "update_error: {}",
        state
            .error_message
            .as_deref()
            .map(redaction::redact_for_persistence)
            .as_deref()
            .unwrap_or("none")
    )
}

fn run_prompt_install_cli(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
    cli_path: Option<PathBuf>,
    print_path: bool,
) -> Result<()> {
    let outcome = prompt_install_cli(config, state, paths, cli_path)?;
    match outcome {
        PromptInstallCliOutcome::Installed(path) => {
            if print_path {
                println!("{}", path.display());
            }
            std::process::exit(0);
        }
        PromptInstallCliOutcome::Cancelled => {
            std::process::exit(PROMPT_INSTALL_CLI_CANCELLED_EXIT_CODE);
        }
        PromptInstallCliOutcome::NoBackend => {
            std::process::exit(PROMPT_INSTALL_CLI_NO_BACKEND_EXIT_CODE);
        }
    }
}

fn run_cli_preflight(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
    cli_path: Option<std::path::PathBuf>,
    print_path: bool,
    allow_install_missing: bool,
) -> Result<()> {
    let outcome = codex_cli::preflight(config, state, paths, cli_path, allow_install_missing)?;
    if print_path {
        println!("{}", outcome.cli_path.display());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PromptInstallCliOutcome {
    Installed(PathBuf),
    Cancelled,
    NoBackend,
}

fn prompt_install_cli(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
    cli_path: Option<PathBuf>,
) -> Result<PromptInstallCliOutcome> {
    if let Some(path) = cli_path
        .as_deref()
        .and_then(|path| codex_cli::resolve_cli_path(Some(path)))
        .or_else(|| {
            state
                .cli_path
                .as_deref()
                .and_then(|path| codex_cli::resolve_cli_path(Some(path)))
        })
        .or_else(|| {
            config
                .cli_path
                .as_deref()
                .and_then(|path| codex_cli::resolve_cli_path(Some(path)))
        })
        .or_else(|| codex_cli::resolve_cli_path(None))
    {
        return Ok(PromptInstallCliOutcome::Installed(path));
    }

    if recently_dismissed_cli_prompt(state) {
        return Ok(PromptInstallCliOutcome::Cancelled);
    }

    if !has_graphical_session() {
        return Ok(PromptInstallCliOutcome::NoBackend);
    }

    let consent = if prefers_kdialog() && command_in_path("kdialog").is_some() {
        run_kdialog_prompt()?
    } else if command_in_path("zenity").is_some() {
        run_zenity_prompt()?
    } else if command_in_path("kdialog").is_some() {
        run_kdialog_prompt()?
    } else {
        run_actionable_notification_prompt()?
    };

    if !consent {
        state.cli_prompt_dismissed_at = Some(Utc::now());
        persist_state(paths, state)?;
        return Ok(PromptInstallCliOutcome::Cancelled);
    }

    state.cli_prompt_dismissed_at = None;
    let outcome = codex_cli::preflight(config, state, paths, cli_path, true)?;
    Ok(PromptInstallCliOutcome::Installed(outcome.cli_path))
}

fn recently_dismissed_cli_prompt(state: &PersistedState) -> bool {
    state.cli_prompt_dismissed_at.is_some_and(|dismissed_at| {
        let elapsed = Utc::now().signed_duration_since(dismissed_at);
        elapsed >= ChronoDuration::zero() && elapsed < CLI_MISSING_PROMPT_DISMISS_TTL
    })
}

fn has_graphical_session() -> bool {
    let has_display =
        std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some();
    let has_dbus = std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some()
        || std::env::var_os("XDG_RUNTIME_DIR").is_some();
    has_display && has_dbus
}

fn prefers_kdialog() -> bool {
    desktop_tokens().iter().any(|token| {
        matches!(
            token.as_str(),
            "kde" | "plasma" | "plasmawayland" | "plasmax11"
        )
    })
}

fn desktop_tokens() -> Vec<String> {
    [
        std::env::var("XDG_CURRENT_DESKTOP").ok(),
        std::env::var("DESKTOP_SESSION").ok(),
    ]
    .into_iter()
    .flatten()
    .flat_map(|value| {
        value
            .split(':')
            .map(|segment| segment.trim().to_ascii_lowercase())
            .collect::<Vec<_>>()
    })
    .filter(|token| !token.is_empty())
    .collect()
}

fn command_in_path(name: &str) -> Option<PathBuf> {
    let path_env = std::env::var_os("PATH").unwrap_or_else(|| OsString::from(""));
    std::env::split_paths(&path_env).find_map(|entry| {
        let candidate = entry.join(name);
        if is_executable_file(&candidate) {
            Some(candidate)
        } else {
            None
        }
    })
}

fn is_executable_file(path: &Path) -> bool {
    path.is_file()
        && path
            .metadata()
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

fn run_kdialog_prompt() -> Result<bool> {
    let status = Command::new("kdialog")
        .args([
            "--title",
            "Codex App",
            "--yesno",
            "Codex CLI is not installed. Install it now?",
        ])
        .status()
        .context("Failed to launch kdialog")?;
    Ok(status.success())
}

fn run_zenity_prompt() -> Result<bool> {
    let status = Command::new("zenity")
        .args([
            "--question",
            "--title=Codex App",
            "--text=Codex CLI is not installed. Install it now?",
        ])
        .status()
        .context("Failed to launch zenity")?;
    Ok(status.success())
}

fn run_actionable_notification_prompt() -> Result<bool> {
    match notify::send_actionable(
        "Codex CLI not installed",
        "Codex App needs the Codex CLI. Choose Install now to let Codex App install it.",
        &[("install", "Install now"), ("dismiss", "Dismiss")],
    )? {
        notify::ActionResponse::Invoked(action) if action == "install" => Ok(true),
        _ => Ok(false),
    }
}

async fn run_check_cycle(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<()> {
    // Keep wrapper state fresh even while a DMG package is pending; otherwise
    // `status --json` could keep advertising stale wrapper candidates.
    if let Err(error) = detect_and_record_wrapper_update(config, state, paths) {
        warn!(?error, "wrapper update detection failed during check cycle");
    }

    if update_install_is_pending(&state.status) {
        info!("skipping remote DMG check because an update is already pending");
        return Ok(());
    }

    reconcile_cli_if_present_best_effort(config, state, paths, "check cycle");

    let retrying_failed_update = state.status == UpdateStatus::Failed;

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
        let metadata = dmg_source::fetch_remote_metadata(&client, &config.dmg_url).await?;
        let previous_headers_fingerprint = state.remote_headers_fingerprint.clone();
        state.remote_headers_fingerprint = Some(metadata.headers_fingerprint.clone());
        state.last_successful_check_at = Some(Utc::now());

        if previous_headers_fingerprint.as_deref() == Some(metadata.headers_fingerprint.as_str())
            && state.dmg_sha256.is_some()
            && state_has_verified_current_dmg(state)
            && !retrying_failed_update
        {
            set_status(state, paths, UpdateStatus::Idle)?;
            info!("official DMG fingerprint unchanged; skipping download");
            return Ok(());
        }

        set_status(state, paths, UpdateStatus::DownloadingDmg)?;

        let downloads_dir = config.workspace_root.join("downloads");
        let downloaded =
            dmg_source::download_dmg(&client, &config.dmg_url, &downloads_dir, Utc::now()).await?;
        let manifest_path = trust::trusted_dmg_manifest_path(&config.builder_bundle_root);
        let verified = match trust::verify_downloaded_dmg_with_manifest(
            &manifest_path,
            &config.dmg_url,
            &downloaded.sha256,
        ) {
            Ok(verified) => {
                record_verified_dmg(state, &verified);
                info!(
                    candidate_version = %verified.version,
                    dmg_sha256 = %verified.sha256,
                    manifest = %verified.manifest_path.display(),
                    "verified downloaded DMG against repo-trusted metadata"
                );
                verified
            }
            Err(error) => {
                record_failed_dmg_verification(
                    state,
                    &downloaded.sha256,
                    manifest_path,
                    error.to_string(),
                );
                return Err(error);
            }
        };

        if downloaded_dmg_is_blocked_by_rollback(
            state,
            &verified.version,
            &downloaded.candidate_version,
            &downloaded.sha256,
        ) {
            state.candidate_version = None;
            state.dmg_sha256 = Some(verified.sha256.clone());
            state.artifact_paths.dmg_path = Some(downloaded.path.clone());
            state.status = UpdateStatus::Idle;
            state.error_message = Some(format!(
                "Candidate {} was rolled back and will not be reinstalled automatically",
                verified.version
            ));
            persist_state(paths, state)?;
            info!(
                candidate_version = %verified.version,
                "skipping candidate blocked by rollback"
            );
            return Ok(());
        }

        if state.dmg_sha256.as_deref() == Some(downloaded.sha256.as_str())
            && !retrying_failed_update
        {
            state.status = UpdateStatus::Idle;
            state.artifact_paths.dmg_path = Some(downloaded.path);
            persist_state(paths, state)?;
            info!("downloaded DMG hash matches current cached DMG; no update detected");
            return Ok(());
        }

        rollback::record_current_package_as_known_good(state);
        state.status = UpdateStatus::UpdateDetected;
        state.candidate_version = Some(verified.version.clone());
        state.dmg_sha256 = Some(verified.sha256.clone());
        state.artifact_paths.dmg_path = Some(downloaded.path.clone());
        state.notified_events.clear();
        state.save(&paths.state_file)?;

        maybe_notify(
            state,
            paths,
            config.notifications,
            "update_detected",
            "New Codex App update detected",
            "Preparing a local Linux package from the new official OpenAI Codex DMG.",
        )?;

        let candidate_version = state
            .candidate_version
            .clone()
            .expect("candidate version should be set before local build");
        ensure_downloaded_dmg_still_matches_verified_metadata(&downloaded.path, &verified).await?;
        builder::build_update(config, state, paths, &candidate_version, &downloaded.path).await?;
        if state.candidate_version.as_deref() != Some(verified.version.as_str()) {
            let message = format!(
                "Built package version {} did not match trusted DMG metadata version {}",
                state.candidate_version.as_deref().unwrap_or("unknown"),
                verified.version
            );
            mark_failed_and_persist(state, paths, message.clone())?;
            return Err(anyhow::anyhow!(message));
        }
        maybe_notify_update_ready(state, paths, config.notifications)?;
        Ok(())
    }
    .await;

    if let Err(error) = result {
        mark_failed_and_persist(state, paths, error.to_string())?;
        maybe_prune_workspace_cache(&config.workspace_root, state);
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
    recover_interrupted_install(&config.workspace_root, state, paths)?;
    if complete_pending_install_if_already_installed(&config.workspace_root, state, paths)? {
        let _ = maybe_notify_installed(state, paths, config.notifications);
        return Ok(());
    }

    match state.status {
        UpdateStatus::ReadyToInstall => {
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

            if let Err(error) = ensure_ready_update_has_verified_dmg(state) {
                mark_failed_and_persist(state, paths, error.to_string())?;
                return Ok(());
            }
            if let Err(error) =
                expected_package_for_ready_install(state, &config.workspace_root, &package_path)
            {
                mark_failed_and_persist(state, paths, error.to_string())?;
                return Ok(());
            }

            // The persisted `auto_install_on_app_exit` key is kept for config
            // compatibility. In this flow it controls whether the updater
            // nudges a running app; installation still requires an explicit
            // install-ready trigger from the app/menu path.
            if state.auto_install_on_app_exit && liveness::is_app_running(config)? {
                clear_install_auth_required_event(state, paths)?;
                set_waiting_for_app_exit(state, paths, true)?;
                maybe_notify(
                    state,
                    paths,
                    config.notifications,
                    "ready_to_install",
                    "Codex App update ready",
                    "Open Codex App and choose Update to install the ready update.",
                )?;
                return Ok(());
            }

            set_status(state, paths, UpdateStatus::ReadyToInstall)?;
        }
        UpdateStatus::WaitingForAppExit => {
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

            if let Err(error) = ensure_ready_update_has_verified_dmg(state) {
                mark_failed_and_persist(state, paths, error.to_string())?;
                return Ok(());
            }
            let expected_package = match expected_package_for_ready_install(
                state,
                &config.workspace_root,
                &package_path,
            ) {
                Ok(expected) => expected,
                Err(error) => {
                    mark_failed_and_persist(state, paths, error.to_string())?;
                    return Ok(());
                }
            };
            if state.waiting_for_app_exit_auto_install && !state.auto_install_on_app_exit {
                set_status(state, paths, UpdateStatus::ReadyToInstall)?;
                return Ok(());
            }

            if liveness::is_app_running(config)? {
                clear_install_auth_required_event(state, paths)?;
                maybe_notify(
                    state,
                    paths,
                    config.notifications,
                    "waiting_for_app_exit",
                    "Codex App update ready",
                    "An update is ready and will install after you close Codex App.",
                )?;
                return Ok(());
            }

            if install_auth_retry_is_blocked(state) {
                return Ok(());
            }

            trigger_install(
                state,
                paths,
                &config.workspace_root,
                &package_path,
                &expected_package,
            )
            .await?;
        }
        _ => {}
    }

    Ok(())
}

async fn run_install_ready(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<()> {
    sync_and_persist(config, state, paths)?;
    let recovering_interrupted_install = state.status == UpdateStatus::Installing;
    recover_interrupted_install(&config.workspace_root, state, paths)?;
    if recovering_interrupted_install && state.status == UpdateStatus::Failed {
        let message = state
            .error_message
            .clone()
            .unwrap_or_else(|| "Previous install attempt could not be recovered".to_string());
        maybe_send_notification(
            config.notifications,
            "Codex update failed",
            "The previous install attempt could not be recovered. Check the updater log for details.",
        );
        return Err(anyhow::anyhow!(message));
    }

    if complete_pending_install_if_already_installed(&config.workspace_root, state, paths)? {
        let _ = maybe_notify_installed(state, paths, config.notifications);
        println!("Codex App update is already installed or superseded.");
        return Ok(());
    }

    match state.status {
        UpdateStatus::ReadyToInstall | UpdateStatus::WaitingForAppExit => {}
        UpdateStatus::Installing => {
            maybe_send_notification(
                config.notifications,
                "Codex update already installing",
                "Codex App is already applying the ready update.",
            );
            println!("Codex App update is already installing.");
            return Ok(());
        }
        _ => {
            maybe_send_notification(
                config.notifications,
                "No Codex update ready",
                "There is no rebuilt Codex App update waiting to install.",
            );
            println!("No Codex App update is ready to install.");
            return Ok(());
        }
    }

    let Some(package_path) = state.artifact_paths.package_path.clone() else {
        let message = "No ready update package is recorded";
        mark_failed_and_persist(state, paths, message)?;
        maybe_send_notification(
            config.notifications,
            "Codex update failed",
            "The updater has no package path recorded for the ready update.",
        );
        return Err(anyhow::anyhow!(message));
    };

    if !package_path.exists() {
        let message = format!(
            "Pending package artifact is missing: {}",
            package_path.display()
        );
        mark_failed_and_persist(state, paths, message.clone())?;
        maybe_send_notification(
            config.notifications,
            "Codex update failed",
            "The rebuilt package is missing. Check the updater log for details.",
        );
        return Err(anyhow::anyhow!(message));
    }

    if let Err(error) = ensure_ready_update_has_verified_dmg(state) {
        let message = error.to_string();
        mark_failed_and_persist(state, paths, message.clone())?;
        maybe_send_notification(config.notifications, "Codex update failed", &message);
        return Err(anyhow::anyhow!(message));
    }
    let expected_package =
        match expected_package_for_ready_install(state, &config.workspace_root, &package_path) {
            Ok(expected) => expected,
            Err(error) => {
                let message = error.to_string();
                mark_failed_and_persist(state, paths, message.clone())?;
                maybe_send_notification(config.notifications, "Codex update failed", &message);
                return Err(anyhow::anyhow!(message));
            }
        };

    if liveness::is_app_running(config)? {
        clear_install_auth_required_event(state, paths)?;
        set_waiting_for_app_exit(state, paths, false)?;
        maybe_send_notification(
            config.notifications,
            "Codex App update ready",
            "Close Codex App to install the ready update.",
        );
        println!("Codex App is running. Close it to install the ready update.");
        return Ok(());
    }

    clear_install_auth_required_event(state, paths)?;
    trigger_install(
        state,
        paths,
        &config.workspace_root,
        &package_path,
        &expected_package,
    )
    .await
}

fn expected_package_for_ready_install(
    state: &PersistedState,
    workspace_root: &Path,
    package_path: &Path,
) -> Result<install::ExpectedPackage> {
    package_verification::expected_package_for_ready_install(
        package_path,
        workspace_root,
        state.candidate_version.as_deref(),
        state.dmg_sha256.as_deref(),
        state.package_verification.as_ref(),
    )
}

fn complete_pending_install_if_already_installed(
    workspace_root: &Path,
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<bool> {
    if !matches!(
        state.status,
        UpdateStatus::ReadyToInstall | UpdateStatus::WaitingForAppExit
    ) {
        return Ok(false);
    }

    let Some(candidate_version) = state.candidate_version.clone().filter(|candidate| {
        installed_version_satisfies_candidate(&state.installed_version, candidate)
    }) else {
        return Ok(false);
    };

    let candidate_is_installed =
        installed_version_matches_candidate(&state.installed_version, &candidate_version);
    if candidate_is_installed
        && state_has_verified_current_dmg(state)
        && state
            .artifact_paths
            .package_path
            .as_ref()
            .is_some_and(|package_path| package_path.exists())
    {
        return Ok(false);
    }

    state.status = UpdateStatus::Installed;
    state.waiting_for_app_exit_auto_install = false;
    state.candidate_version = None;
    if !candidate_is_installed {
        state.artifact_paths.package_path = None;
        state.package_verification = None;
    }
    state.error_message = None;
    state.notified_events.clear();
    cache_cleanup::normalize_artifact_workspace_dir(workspace_root, state);
    persist_state(paths, state)?;
    maybe_prune_workspace_cache(workspace_root, state);
    info!("recovered pending install state because the candidate version is already installed or superseded");
    Ok(true)
}

fn recover_interrupted_install(
    workspace_root: &Path,
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<()> {
    if state.status != UpdateStatus::Installing {
        return Ok(());
    }

    if let Some(candidate_version) = state.candidate_version.clone().filter(|candidate| {
        installed_version_satisfies_candidate(&state.installed_version, candidate)
    }) {
        let candidate_is_installed =
            installed_version_matches_candidate(&state.installed_version, &candidate_version);

        state.status = UpdateStatus::Installed;
        state.waiting_for_app_exit_auto_install = false;
        state.candidate_version = None;
        if !candidate_is_installed {
            state.artifact_paths.package_path = None;
            state.package_verification = None;
        }
        state.error_message = None;
        state.notified_events.clear();
        cache_cleanup::normalize_artifact_workspace_dir(workspace_root, state);
        persist_state(paths, state)?;
        maybe_prune_workspace_cache(workspace_root, state);
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
    state.waiting_for_app_exit_auto_install = false;
    state.error_message =
        Some("Previous install attempt was interrupted before completion".to_string());
    cache_cleanup::normalize_artifact_workspace_dir(workspace_root, state);
    persist_state(paths, state)?;
    info!(package = %package_path.display(), "recovered interrupted install state back to ready_to_install");
    Ok(())
}

fn installed_version_satisfies_candidate(installed: &str, candidate: &str) -> bool {
    if installed == "unknown" {
        return false;
    }

    match compare_generated_versions(installed, candidate) {
        Some(std::cmp::Ordering::Less) => false,
        Some(_) => true,
        None => installed == candidate,
    }
}

fn installed_version_matches_candidate(installed: &str, candidate: &str) -> bool {
    match compare_generated_versions(installed, candidate) {
        Some(std::cmp::Ordering::Equal) => true,
        Some(_) => false,
        None => installed == candidate,
    }
}

fn compare_generated_versions(left: &str, right: &str) -> Option<std::cmp::Ordering> {
    let left = parse_generated_version(left)?;
    let right = parse_generated_version(right)?;
    Some(left.cmp(&right))
}

fn parse_generated_version(version: &str) -> Option<Vec<u32>> {
    let without_metadata = version
        .split_once('+')
        .map(|(prefix, _)| prefix)
        .unwrap_or(version);
    let base = without_metadata
        .split_once('-')
        .map(|(prefix, _)| prefix)
        .unwrap_or(without_metadata);
    let mut parts = Vec::new();
    for segment in base.split('.') {
        parts.push(segment.parse::<u32>().ok()?);
    }
    if !matches!(parts.len(), 3 | 4) {
        return None;
    }
    Some(parts)
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
    state.cli_status == CliStatus::NotInstalled
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
        "Codex App needs the Codex CLI. Open the app to retry the automatic install flow, or install it manually with npm.",
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
        "Codex App updated",
        "The new package is installed and will be used the next time you open the app.",
    )
}

fn maybe_notify_update_ready(
    state: &mut PersistedState,
    paths: &RuntimePaths,
    enabled: bool,
) -> Result<()> {
    let version = state
        .candidate_version
        .as_deref()
        .unwrap_or(&state.installed_version);
    let event_key = format!("ready_to_install:{version}");
    if !state.notified_events.insert(event_key) {
        return Ok(());
    }

    if enabled {
        let body = if state.auto_install_on_app_exit {
            "A rebuilt Linux package is ready. Close Codex App to install it, or open Codex App and choose Update."
        } else {
            "A rebuilt Linux package is ready. Open Codex App and choose Update to install it."
        };
        if let Err(error) = notify::send("Codex App update ready", body) {
            warn!(?error, "failed to send update-ready notification");
        }
    }

    persist_state(paths, state)?;
    Ok(())
}

fn maybe_send_notification(enabled: bool, summary: &str, body: &str) {
    if enabled {
        let _ = notify::send(summary, body);
    }
}

fn downloaded_dmg_is_blocked_by_rollback(
    state: &PersistedState,
    trusted_version: &str,
    legacy_download_candidate_version: &str,
    dmg_sha256: &str,
) -> bool {
    state
        .rollback_blocked_dmg_sha256
        .as_deref()
        .is_some_and(|blocked| blocked == dmg_sha256)
        || state
            .rollback_blocked_candidate_version
            .as_deref()
            .is_some_and(|blocked| {
                rollback_version_satisfies_candidate(blocked, trusted_version)
                    || rollback_version_satisfies_candidate(
                        blocked,
                        legacy_download_candidate_version,
                    )
            })
}

fn rollback_version_satisfies_candidate(blocked: &str, candidate: &str) -> bool {
    match (
        parse_generated_version(blocked),
        parse_generated_version(candidate),
    ) {
        (Some(blocked_parts), Some(candidate_parts))
            if blocked_parts.len() == 3 && candidate_parts.len() == 3 =>
        {
            false
        }
        (Some(blocked_parts), Some(candidate_parts))
            if blocked_parts.len() == candidate_parts.len() =>
        {
            installed_version_satisfies_candidate(blocked, candidate)
        }
        (Some(_), Some(_)) => false,
        _ => blocked == candidate,
    }
}

async fn trigger_install(
    state: &mut PersistedState,
    paths: &RuntimePaths,
    workspace_root: &Path,
    package_path: &Path,
    expected_package: &install::ExpectedPackage,
) -> Result<()> {
    state.status = UpdateStatus::Installing;
    state.waiting_for_app_exit_auto_install = false;
    state.error_message = None;
    persist_state(paths, state)?;

    let _ = notify::send(
        "Installing Codex App update",
        "Applying the locally rebuilt Linux package.",
    );

    let output = install::pkexec_command(package_path, Some(expected_package))?
        .output()
        .context("Failed to launch pkexec for update installation")?;
    let status = output.status;

    if status.success() {
        state.status = UpdateStatus::Installed;
        state.waiting_for_app_exit_auto_install = false;
        state.installed_version = install::installed_package_version();
        state.candidate_version = None;
        state.rollback_blocked_candidate_version = None;
        state.rollback_blocked_dmg_sha256 = None;
        state.error_message = None;
        state.notified_events.clear();
        cache_cleanup::normalize_artifact_workspace_dir(workspace_root, state);
        persist_state(paths, state)?;
        let _ = maybe_notify_installed(state, paths, true);
        maybe_prune_workspace_cache(workspace_root, state);
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
    if pkexec_authentication_was_not_obtained(&status) {
        defer_install_until_next_app_exit(state, paths, error.to_string())?;
        return Err(error);
    }

    mark_failed_and_persist(state, paths, error.to_string())?;
    let _ = notify::send(
        "Codex update failed",
        "The package could not be installed. Check the updater log for details.",
    );
    Err(error)
}

fn pkexec_authentication_was_not_obtained(status: &std::process::ExitStatus) -> bool {
    matches!(status.code(), Some(126 | 127))
}

fn install_auth_required_event_key(state: &PersistedState) -> Option<String> {
    state
        .candidate_version
        .as_deref()
        .map(|candidate| format!("install_auth_required:{candidate}"))
}

fn install_auth_retry_is_blocked(state: &PersistedState) -> bool {
    install_auth_required_event_key(state)
        .as_ref()
        .is_some_and(|event_key| state.notified_events.contains(event_key))
}

fn clear_install_auth_required_event(
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<()> {
    let Some(event_key) = install_auth_required_event_key(state) else {
        return Ok(());
    };

    if state.notified_events.remove(&event_key) {
        persist_state(paths, state)?;
    }

    Ok(())
}

fn defer_install_until_next_app_exit(
    state: &mut PersistedState,
    paths: &RuntimePaths,
    message: String,
) -> Result<()> {
    state.status = UpdateStatus::ReadyToInstall;
    state.waiting_for_app_exit_auto_install = false;
    state.error_message = Some(message);

    if let Some(event_key) = install_auth_required_event_key(state) {
        if state.notified_events.insert(event_key) {
            let _ = notify::send(
                "Codex update needs permission",
                "The ready update will retry after the next app close. Approve the system authentication dialog to install it.",
            );
        }
    }

    persist_state(paths, state)
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

    const TRUSTED_TEST_DMG_SHA256: &str =
        "6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a";

    fn mark_test_dmg_verified(state: &mut PersistedState, version: &str) {
        state.dmg_sha256 = Some(TRUSTED_TEST_DMG_SHA256.to_string());
        state.dmg_verification = Some(DmgVerification {
            result: DmgVerificationResult::Verified,
            version: Some(version.to_string()),
            sha256: Some(TRUSTED_TEST_DMG_SHA256.to_string()),
            manifest_path: Some(PathBuf::from(
                "/usr/lib/codex-app/update-builder/updater/trusted-dmg-manifest.json",
            )),
            verified_at: Some(Utc::now()),
            message: Some("Downloaded DMG matched repo-trusted metadata".to_string()),
        });
    }

    fn write_test_package_and_verification(
        state: &mut PersistedState,
        workspace_root: &Path,
        version: &str,
    ) -> Result<PathBuf> {
        let workspace = workspace_root.join("workspaces").join(version);
        let package_path = workspace.join("dist/codex.deb");
        std::fs::create_dir_all(
            package_path
                .parent()
                .expect("package path should have parent"),
        )?;
        std::fs::write(&package_path, b"deb")?;
        state.package_verification = Some(package_verification::record_built_package(
            &package_path,
            &workspace,
            version,
            TRUSTED_TEST_DMG_SHA256,
        )?);
        Ok(package_path)
    }

    fn test_paths(root: &std::path::Path) -> RuntimePaths {
        RuntimePaths {
            config_file: root.join("config/config.toml"),
            state_file: root.join("state/state.json"),
            log_file: root.join("state/service.log"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            config_dir: root.join("config"),
        }
    }

    fn test_config(root: &std::path::Path) -> RuntimeConfig {
        RuntimeConfig {
            dmg_url: "https://example.com/Codex.dmg".to_string(),
            initial_check_delay_seconds: 1,
            check_interval_hours: 6,
            auto_install_on_app_exit: true,
            notifications: false,
            developer_mode: false,
            workspace_root: root.join("cache"),
            builder_bundle_root: root.join("builder"),
            app_executable_path: root.join("not-running-electron"),
            cli_path: None,
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        }
    }

    #[test]
    fn remote_dmg_check_freshness_respects_configured_interval() {
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
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(true);
        assert!(!remote_dmg_check_is_fresh(&config, &state));

        state.last_successful_check_at = Some(Utc::now() - ChronoDuration::hours(1));
        assert!(remote_dmg_check_is_fresh(&config, &state));

        state.last_successful_check_at = Some(Utc::now() - ChronoDuration::hours(7));
        assert!(!remote_dmg_check_is_fresh(&config, &state));
    }

    #[test]
    fn disabled_wrapper_tracking_clears_stale_candidate() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = test_paths(temp.path());
        paths.ensure_dirs()?;
        let config = test_config(temp.path());

        let mut state = PersistedState::new(true);
        state.installed_wrapper_commit = Some("installed".to_string());
        state.candidate_wrapper_commit = Some("stale".to_string());
        state.candidate_wrapper_version = Some("0.9.0".to_string());
        state.wrapper_changelog = Some("old changelog".to_string());
        state.wrapper_dev_mode = Some(true);

        let found = detect_and_record_wrapper_update(&config, &mut state, &paths)?;

        assert!(!found);
        assert_eq!(state.installed_wrapper_commit.as_deref(), Some("installed"));
        assert_eq!(state.candidate_wrapper_commit, None);
        assert_eq!(state.candidate_wrapper_version, None);
        assert_eq!(state.wrapper_changelog, None);
        assert_eq!(state.wrapper_dev_mode, None);

        let persisted = PersistedState::load_or_default(&paths.state_file, true)?;
        assert_eq!(persisted.candidate_wrapper_commit, None);
        assert_eq!(persisted.wrapper_changelog, None);
        assert_eq!(persisted.wrapper_dev_mode, None);
        Ok(())
    }

    #[test]
    fn no_wrapper_update_clears_stale_candidate() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = test_paths(temp.path());
        paths.ensure_dirs()?;
        let mut config = test_config(temp.path());
        config.enable_wrapper_updates = true;
        std::fs::create_dir_all(&config.builder_bundle_root)?;

        let mut state = PersistedState::new(true);
        state.installed_wrapper_commit = Some("old-installed".to_string());
        state.candidate_wrapper_commit = Some("stale".to_string());
        state.candidate_wrapper_version = Some("0.9.0".to_string());
        state.wrapper_changelog = Some("old changelog".to_string());
        state.wrapper_dev_mode = Some(true);

        let found = detect_and_record_wrapper_update(&config, &mut state, &paths)?;

        assert!(!found);
        assert_eq!(state.installed_wrapper_commit, None);
        assert_eq!(state.installed_wrapper_version, None);
        assert_eq!(state.candidate_wrapper_commit, None);
        assert_eq!(state.candidate_wrapper_version, None);
        assert_eq!(state.wrapper_changelog, None);
        assert_eq!(state.wrapper_dev_mode, None);

        let persisted = PersistedState::load_or_default(&paths.state_file, true)?;
        assert_eq!(persisted.installed_wrapper_commit, None);
        assert_eq!(persisted.candidate_wrapper_commit, None);
        assert_eq!(persisted.wrapper_changelog, None);
        assert_eq!(persisted.wrapper_dev_mode, None);
        Ok(())
    }

    #[test]
    fn unknown_wrapper_detection_clears_stale_candidate_but_records_installed_metadata(
    ) -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = test_paths(temp.path());
        paths.ensure_dirs()?;
        let mut config = test_config(temp.path());
        config.enable_wrapper_updates = true;
        std::fs::create_dir_all(config.builder_bundle_root.join(".codex-linux"))?;
        std::fs::write(
            config
                .builder_bundle_root
                .join(".codex-linux/source-info.json"),
            r#"{
  "commit": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "version": "0.8.1"
}
"#,
        )?;

        let mut state = PersistedState::new(true);
        state.candidate_wrapper_commit = Some("stale".to_string());
        state.candidate_wrapper_version = Some("0.9.0".to_string());
        state.wrapper_changelog = Some("old changelog".to_string());
        state.wrapper_dev_mode = Some(true);

        let found = detect_and_record_wrapper_update(&config, &mut state, &paths)?;

        assert!(!found);
        assert_eq!(
            state.installed_wrapper_commit.as_deref(),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(state.installed_wrapper_version.as_deref(), Some("0.8.1"));
        assert_eq!(state.candidate_wrapper_commit, None);
        assert_eq!(state.candidate_wrapper_version, None);
        assert_eq!(state.wrapper_changelog, None);
        assert_eq!(state.wrapper_dev_mode, None);
        Ok(())
    }

    #[tokio::test]
    async fn pending_dmg_update_still_clears_stale_wrapper_candidate() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = test_paths(temp.path());
        paths.ensure_dirs()?;
        let config = test_config(temp.path());

        let mut state = PersistedState::new(true);
        state.status = UpdateStatus::ReadyToInstall;
        state.candidate_wrapper_commit = Some("stale".to_string());
        state.candidate_wrapper_version = Some("0.9.0".to_string());
        state.wrapper_changelog = Some("old changelog".to_string());
        state.wrapper_dev_mode = Some(true);

        run_check_cycle(&config, &mut state, &paths).await?;

        assert_eq!(state.status, UpdateStatus::ReadyToInstall);
        assert_eq!(state.candidate_wrapper_commit, None);
        assert_eq!(state.candidate_wrapper_version, None);
        assert_eq!(state.wrapper_changelog, None);
        assert_eq!(state.wrapper_dev_mode, None);
        let persisted = PersistedState::load_or_default(&paths.state_file, true)?;
        assert_eq!(persisted.candidate_wrapper_commit, None);
        assert_eq!(persisted.wrapper_changelog, None);
        assert_eq!(persisted.wrapper_dev_mode, None);
        Ok(())
    }

    #[tokio::test]
    async fn fresh_check_now_still_clears_stale_wrapper_candidate() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let paths = test_paths(temp.path());
        paths.ensure_dirs()?;
        let config = test_config(temp.path());

        let mut state = PersistedState::new(true);
        state.last_successful_check_at = Some(Utc::now());
        state.candidate_wrapper_commit = Some("stale".to_string());
        state.candidate_wrapper_version = Some("0.9.0".to_string());
        state.wrapper_changelog = Some("old changelog".to_string());
        state.wrapper_dev_mode = Some(true);

        run_check_now(&config, &mut state, &paths, true).await?;

        assert_eq!(state.status, UpdateStatus::Idle);
        assert_eq!(state.candidate_wrapper_commit, None);
        assert_eq!(state.candidate_wrapper_version, None);
        assert_eq!(state.wrapper_changelog, None);
        assert_eq!(state.wrapper_dev_mode, None);
        let persisted = PersistedState::load_or_default(&paths.state_file, true)?;
        assert_eq!(persisted.candidate_wrapper_commit, None);
        assert_eq!(persisted.wrapper_changelog, None);
        assert_eq!(persisted.wrapper_dev_mode, None);
        Ok(())
    }

    #[test]
    fn plain_status_reports_update_error() {
        let mut state = PersistedState::new(true);
        state.status = UpdateStatus::Failed;
        state.error_message = Some("install.sh failed during local rebuild".to_string());

        assert_eq!(
            update_error_status_line(&state),
            "update_error: install.sh failed during local rebuild"
        );

        state.error_message = None;
        assert_eq!(update_error_status_line(&state), "update_error: none");
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

        let config = RuntimeConfig {
            dmg_url: "https://example.com/Codex.dmg".to_string(),
            initial_check_delay_seconds: 1,
            check_interval_hours: 6,
            auto_install_on_app_exit: false,
            notifications: false,
            developer_mode: false,
            workspace_root: temp.path().join("cache"),
            builder_bundle_root: temp.path().join("builder"),
            app_executable_path: std::env::current_exe()?,
            cli_path: None,
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(false);
        state.status = UpdateStatus::Failed;
        state.candidate_version = Some("2999.03.25.010203+deadbeef".to_string());
        state.error_message = Some("previous failure".to_string());
        let package_path = temp.path().join("dist/codex.deb");
        std::fs::create_dir_all(
            package_path
                .parent()
                .expect("package path should have parent"),
        )?;
        std::fs::write(&package_path, b"deb")?;
        state.artifact_paths.package_path = Some(package_path);

        reconcile_pending_install(&config, &mut state, &paths).await?;

        assert_eq!(state.status, UpdateStatus::Failed);
        assert_eq!(state.error_message.as_deref(), Some("previous failure"));
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
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
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

    #[tokio::test]
    async fn run_check_cycle_fails_when_downloaded_dmg_has_no_trusted_metadata() -> Result<()> {
        use wiremock::{
            matchers::{method, path},
            Mock, MockServer, ResponseTemplate,
        };

        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .and(path("/Codex.dmg"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("ETag", "\"untrusted\"")
                    .insert_header("Content-Length", "13"),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/Codex.dmg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"untrusted-dmg".to_vec()))
            .mount(&server)
            .await;

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
            dmg_url: format!("{}/Codex.dmg", server.uri()),
            initial_check_delay_seconds: 1,
            check_interval_hours: 6,
            auto_install_on_app_exit: true,
            notifications: false,
            developer_mode: false,
            workspace_root: temp.path().join("cache"),
            builder_bundle_root: temp.path().join("builder"),
            app_executable_path: temp.path().join("not-running-electron"),
            cli_path: None,
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(true);
        let error = run_check_cycle(&config, &mut state, &paths)
            .await
            .expect_err("untrusted DMG should fail before local rebuild");

        assert!(error
            .to_string()
            .contains("Failed to read trusted DMG metadata"));
        assert_eq!(state.status, UpdateStatus::Failed);
        assert!(state
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("Failed to read trusted DMG metadata"));
        assert_eq!(state.artifact_paths.package_path, None);
        Ok(())
    }

    #[tokio::test]
    async fn run_check_cycle_rechecks_unchanged_headers_without_verification_state() -> Result<()> {
        use wiremock::{
            matchers::{method, path},
            Mock, MockServer, ResponseTemplate,
        };

        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .and(path("/Codex.dmg"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("ETag", "\"stable\"")
                    .insert_header("Content-Length", "13"),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/Codex.dmg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"untrusted-dmg".to_vec()))
            .mount(&server)
            .await;

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
            dmg_url: format!("{}/Codex.dmg", server.uri()),
            initial_check_delay_seconds: 1,
            check_interval_hours: 6,
            auto_install_on_app_exit: true,
            notifications: false,
            developer_mode: false,
            workspace_root: temp.path().join("cache"),
            builder_bundle_root: temp.path().join("builder"),
            app_executable_path: temp.path().join("not-running-electron"),
            cli_path: None,
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(true);
        state.remote_headers_fingerprint =
            Some("etag=\"stable\"|last_modified=|content_length=13".to_string());
        state.dmg_sha256 =
            Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string());

        let error = run_check_cycle(&config, &mut state, &paths)
            .await
            .expect_err("state without trusted DMG verification should be rechecked");

        assert!(error
            .to_string()
            .contains("Failed to read trusted DMG metadata"));
        assert_eq!(state.status, UpdateStatus::Failed);
        Ok(())
    }

    #[tokio::test]
    async fn run_check_cycle_rechecks_unchanged_headers_with_incomplete_verification_state(
    ) -> Result<()> {
        use wiremock::{
            matchers::{method, path},
            Mock, MockServer, ResponseTemplate,
        };

        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .and(path("/Codex.dmg"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("ETag", "\"stable\"")
                    .insert_header("Content-Length", "13"),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/Codex.dmg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"untrusted-dmg".to_vec()))
            .mount(&server)
            .await;

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
            dmg_url: format!("{}/Codex.dmg", server.uri()),
            initial_check_delay_seconds: 1,
            check_interval_hours: 6,
            auto_install_on_app_exit: true,
            notifications: false,
            developer_mode: false,
            workspace_root: temp.path().join("cache"),
            builder_bundle_root: temp.path().join("builder"),
            app_executable_path: temp.path().join("not-running-electron"),
            cli_path: None,
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(true);
        state.remote_headers_fingerprint =
            Some("etag=\"stable\"|last_modified=|content_length=13".to_string());
        state.dmg_sha256 = Some(TRUSTED_TEST_DMG_SHA256.to_string());
        state.dmg_verification = Some(DmgVerification {
            result: DmgVerificationResult::Verified,
            version: None,
            sha256: Some(TRUSTED_TEST_DMG_SHA256.to_string()),
            manifest_path: Some(PathBuf::from(
                "/usr/lib/codex-app/update-builder/updater/trusted-dmg-manifest.json",
            )),
            verified_at: Some(Utc::now()),
            message: Some("Downloaded DMG matched repo-trusted metadata".to_string()),
        });

        let error = run_check_cycle(&config, &mut state, &paths)
            .await
            .expect_err("incomplete trusted DMG verification should be rechecked");

        assert!(error
            .to_string()
            .contains("Failed to read trusted DMG metadata"));
        assert_eq!(state.status, UpdateStatus::Failed);
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
    fn held_check_lock_blocks_second_acquire_until_drop() -> Result<()> {
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

        let first_lock =
            try_acquire_check_lock(&paths)?.expect("first lock acquisition should succeed");
        let second_lock = try_acquire_check_lock(&paths)?;

        assert!(second_lock.is_none());
        drop(second_lock);
        drop(first_lock);

        let mut reacquired_lock = None;
        for _ in 0..20 {
            reacquired_lock = try_acquire_check_lock(&paths)?;
            if reacquired_lock.is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert!(reacquired_lock.is_some());
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
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(true);
        state.status = UpdateStatus::ReadyToInstall;
        state.candidate_version = Some("2999.03.25.010203+deadbeef".to_string());
        state.artifact_paths.package_path = Some(temp.path().join("missing/codex.deb"));

        reconcile_pending_install(&config, &mut state, &paths).await?;

        assert_eq!(state.status, UpdateStatus::Failed);
        assert!(state
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains("Pending package artifact is missing")));
        Ok(())
    }

    #[test]
    fn ready_update_waits_for_explicit_install_ready_when_auto_install_is_off() -> Result<()> {
        let _env_guard = crate::test_util::env_lock();
        let runtime = tokio::runtime::Runtime::new()?;
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
        let settings_path = temp.path().join("settings.json");
        let previous_settings_file = std::env::var_os("CODEX_LINUX_SETTINGS_FILE");
        std::env::set_var("CODEX_LINUX_SETTINGS_FILE", &settings_path);
        std::fs::write(
            &settings_path,
            r#"{"codex-linux-auto-update-on-exit": false}"#,
        )?;

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
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(false);
        state.status = UpdateStatus::ReadyToInstall;
        state.candidate_version = Some("2999.03.25.010203".to_string());
        mark_test_dmg_verified(&mut state, "2999.03.25.010203");
        let package_path = write_test_package_and_verification(
            &mut state,
            &config.workspace_root,
            "2999.03.25.010203",
        )?;
        state.artifact_paths.package_path = Some(package_path);

        let result = runtime.block_on(reconcile_pending_install(&config, &mut state, &paths));

        if let Some(value) = previous_settings_file {
            std::env::set_var("CODEX_LINUX_SETTINGS_FILE", value);
        } else {
            std::env::remove_var("CODEX_LINUX_SETTINGS_FILE");
        }

        result?;
        assert_eq!(state.status, UpdateStatus::ReadyToInstall);
        assert_eq!(state.error_message, None);
        Ok(())
    }

    #[test]
    fn ready_update_auto_install_waits_for_app_exit_when_app_is_running() -> Result<()> {
        let _env_guard = crate::test_util::env_lock();
        let runtime = tokio::runtime::Runtime::new()?;
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
        let settings_path = temp.path().join("settings.json");
        let previous_settings_file = std::env::var_os("CODEX_LINUX_SETTINGS_FILE");
        std::env::set_var("CODEX_LINUX_SETTINGS_FILE", &settings_path);
        std::fs::write(
            &settings_path,
            r#"{"codex-linux-auto-update-on-exit": true}"#,
        )?;

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
            auto_install_on_app_exit: true,
            notifications: false,
            developer_mode: false,
            workspace_root: temp.path().join("cache"),
            builder_bundle_root: temp.path().join("builder"),
            app_executable_path: std::env::current_exe()?,
            cli_path: None,
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(true);
        state.status = UpdateStatus::ReadyToInstall;
        state.candidate_version = Some("2999.03.25.010203".to_string());
        mark_test_dmg_verified(&mut state, "2999.03.25.010203");
        let package_path = write_test_package_and_verification(
            &mut state,
            &config.workspace_root,
            "2999.03.25.010203",
        )?;
        state.artifact_paths.package_path = Some(package_path);
        state
            .notified_events
            .insert("install_auth_required:2999.03.25.010203+deadbeef".to_string());

        let result = runtime.block_on(reconcile_pending_install(&config, &mut state, &paths));

        if let Some(value) = previous_settings_file {
            std::env::set_var("CODEX_LINUX_SETTINGS_FILE", value);
        } else {
            std::env::remove_var("CODEX_LINUX_SETTINGS_FILE");
        }

        result?;
        assert_eq!(state.status, UpdateStatus::WaitingForAppExit);
        assert!(state.waiting_for_app_exit_auto_install);
        assert!(!install_auth_retry_is_blocked(&state));
        Ok(())
    }

    #[test]
    fn waiting_for_app_exit_auto_install_cancelled_when_setting_turns_off() -> Result<()> {
        let _env_guard = crate::test_util::env_lock();
        let runtime = tokio::runtime::Runtime::new()?;
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
        let settings_path = temp.path().join("settings.json");
        let previous_settings_file = std::env::var_os("CODEX_LINUX_SETTINGS_FILE");
        std::env::set_var("CODEX_LINUX_SETTINGS_FILE", &settings_path);
        std::fs::write(
            &settings_path,
            r#"{"codex-linux-auto-update-on-exit": false}"#,
        )?;

        let config = RuntimeConfig {
            dmg_url: "https://example.com/Codex.dmg".to_string(),
            initial_check_delay_seconds: 1,
            check_interval_hours: 6,
            auto_install_on_app_exit: true,
            notifications: false,
            developer_mode: false,
            workspace_root: temp.path().join("cache"),
            builder_bundle_root: temp.path().join("builder"),
            app_executable_path: std::env::current_exe()?,
            cli_path: None,
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(true);
        state.status = UpdateStatus::WaitingForAppExit;
        state.waiting_for_app_exit_auto_install = true;
        state.candidate_version = Some("2999.03.25.010203".to_string());
        mark_test_dmg_verified(&mut state, "2999.03.25.010203");
        let package_path = write_test_package_and_verification(
            &mut state,
            &config.workspace_root,
            "2999.03.25.010203",
        )?;
        state.artifact_paths.package_path = Some(package_path);

        let result = runtime.block_on(reconcile_pending_install(&config, &mut state, &paths));

        if let Some(value) = previous_settings_file {
            std::env::set_var("CODEX_LINUX_SETTINGS_FILE", value);
        } else {
            std::env::remove_var("CODEX_LINUX_SETTINGS_FILE");
        }

        result?;
        assert_eq!(state.status, UpdateStatus::ReadyToInstall);
        assert!(!state.auto_install_on_app_exit);
        assert!(!state.waiting_for_app_exit_auto_install);
        assert_eq!(state.error_message, None);
        assert!(state.artifact_paths.package_path.is_some());
        Ok(())
    }

    #[test]
    fn waiting_for_app_exit_manual_install_survives_auto_toggle_off() -> Result<()> {
        let _env_guard = crate::test_util::env_lock();
        let runtime = tokio::runtime::Runtime::new()?;
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
        let settings_path = temp.path().join("settings.json");
        let previous_settings_file = std::env::var_os("CODEX_LINUX_SETTINGS_FILE");
        std::env::set_var("CODEX_LINUX_SETTINGS_FILE", &settings_path);
        std::fs::write(
            &settings_path,
            r#"{"codex-linux-auto-update-on-exit": false}"#,
        )?;

        let config = RuntimeConfig {
            dmg_url: "https://example.com/Codex.dmg".to_string(),
            initial_check_delay_seconds: 1,
            check_interval_hours: 6,
            auto_install_on_app_exit: true,
            notifications: false,
            developer_mode: false,
            workspace_root: temp.path().join("cache"),
            builder_bundle_root: temp.path().join("builder"),
            app_executable_path: std::env::current_exe()?,
            cli_path: None,
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(false);
        state.status = UpdateStatus::WaitingForAppExit;
        state.waiting_for_app_exit_auto_install = false;
        state.candidate_version = Some("2999.03.25.010203".to_string());
        mark_test_dmg_verified(&mut state, "2999.03.25.010203");
        let package_path = write_test_package_and_verification(
            &mut state,
            &config.workspace_root,
            "2999.03.25.010203",
        )?;
        state.artifact_paths.package_path = Some(package_path);

        let result = runtime.block_on(reconcile_pending_install(&config, &mut state, &paths));

        if let Some(value) = previous_settings_file {
            std::env::set_var("CODEX_LINUX_SETTINGS_FILE", value);
        } else {
            std::env::remove_var("CODEX_LINUX_SETTINGS_FILE");
        }

        result?;
        assert_eq!(state.status, UpdateStatus::WaitingForAppExit);
        assert!(!state.auto_install_on_app_exit);
        assert!(!state.waiting_for_app_exit_auto_install);
        assert_eq!(state.error_message, None);
        Ok(())
    }

    #[test]
    fn reconcile_reloads_auto_install_setting_override() -> Result<()> {
        let _env_guard = crate::test_util::env_lock();
        let runtime = tokio::runtime::Runtime::new()?;
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
        let settings_path = temp.path().join("settings.json");

        let previous_settings_file = std::env::var_os("CODEX_LINUX_SETTINGS_FILE");
        std::env::set_var("CODEX_LINUX_SETTINGS_FILE", &settings_path);

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
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(true);

        std::fs::write(
            &settings_path,
            r#"{"codex-linux-auto-update-on-exit": false}"#,
        )?;
        let first_result = runtime.block_on(reconcile_pending_install(&config, &mut state, &paths));
        assert!(!state.auto_install_on_app_exit);

        std::fs::write(
            &settings_path,
            r#"{"codex-linux-auto-update-on-exit": true}"#,
        )?;
        let second_result =
            runtime.block_on(reconcile_pending_install(&config, &mut state, &paths));

        if let Some(value) = previous_settings_file {
            std::env::set_var("CODEX_LINUX_SETTINGS_FILE", value);
        } else {
            std::env::remove_var("CODEX_LINUX_SETTINGS_FILE");
        }

        first_result?;
        second_result?;
        assert!(state.auto_install_on_app_exit);
        Ok(())
    }

    #[tokio::test]
    async fn install_ready_waits_when_app_is_running() -> Result<()> {
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
            auto_install_on_app_exit: false,
            notifications: false,
            developer_mode: false,
            workspace_root: temp.path().join("cache"),
            builder_bundle_root: temp.path().join("builder"),
            app_executable_path: std::env::current_exe()?,
            cli_path: None,
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(false);
        state.status = UpdateStatus::ReadyToInstall;
        state.candidate_version = Some("2999.03.25.010203".to_string());
        mark_test_dmg_verified(&mut state, "2999.03.25.010203");
        let package_path = write_test_package_and_verification(
            &mut state,
            &config.workspace_root,
            "2999.03.25.010203",
        )?;
        state.artifact_paths.package_path = Some(package_path);
        state
            .notified_events
            .insert("install_auth_required:2999.03.25.010203".to_string());

        run_install_ready(&config, &mut state, &paths).await?;

        assert_eq!(state.status, UpdateStatus::WaitingForAppExit);
        assert!(!install_auth_retry_is_blocked(&state));
        Ok(())
    }

    #[tokio::test]
    async fn install_ready_fails_when_ready_update_has_no_trusted_dmg_verification() -> Result<()> {
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
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(false);
        state.status = UpdateStatus::ReadyToInstall;
        state.candidate_version = Some("2999.03.25.010203".to_string());
        state.dmg_sha256 = Some(TRUSTED_TEST_DMG_SHA256.to_string());
        state.artifact_paths.package_path = Some(package_path);

        let error = run_install_ready(&config, &mut state, &paths)
            .await
            .expect_err("ready update without trusted DMG verification should fail");

        assert!(error
            .to_string()
            .contains("Ready update is missing trusted DMG verification"));
        assert_eq!(state.status, UpdateStatus::Failed);
        assert!(state
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("Ready update is missing trusted DMG verification"));
        Ok(())
    }

    #[tokio::test]
    async fn package_verification_missing_ready_update_fails_closed() -> Result<()> {
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
            auto_install_on_app_exit: false,
            notifications: false,
            developer_mode: false,
            workspace_root: temp.path().join("cache"),
            builder_bundle_root: temp.path().join("builder"),
            app_executable_path: temp.path().join("not-running-electron"),
            cli_path: None,
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(false);
        state.status = UpdateStatus::ReadyToInstall;
        state.candidate_version = Some("2999.03.25.010203".to_string());
        mark_test_dmg_verified(&mut state, "2999.03.25.010203");
        let package_path = config
            .workspace_root
            .join("workspaces/2999.03.25.010203/dist/codex.deb");
        std::fs::create_dir_all(
            package_path
                .parent()
                .expect("package path should have parent"),
        )?;
        std::fs::write(&package_path, b"deb")?;
        state.artifact_paths.package_path = Some(package_path);

        let error = run_install_ready(&config, &mut state, &paths)
            .await
            .expect_err("ready update without package verification should fail");

        assert!(error
            .to_string()
            .contains("Ready update package verification is missing"));
        assert_eq!(state.status, UpdateStatus::Failed);
        Ok(())
    }

    #[tokio::test]
    async fn install_ready_fails_when_verified_dmg_record_is_missing_digest() -> Result<()> {
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
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(false);
        state.status = UpdateStatus::ReadyToInstall;
        state.candidate_version = Some("2999.03.25.010203".to_string());
        state.dmg_sha256 = None;
        state.artifact_paths.package_path = Some(package_path);
        state.dmg_verification = Some(DmgVerification {
            result: DmgVerificationResult::Verified,
            version: Some("2999.03.25.010203".to_string()),
            sha256: None,
            manifest_path: Some(PathBuf::from(
                "/usr/lib/codex-app/update-builder/updater/trusted-dmg-manifest.json",
            )),
            verified_at: Some(Utc::now()),
            message: Some("Downloaded DMG matched repo-trusted metadata".to_string()),
        });

        let error = run_install_ready(&config, &mut state, &paths)
            .await
            .expect_err("verified DMG record without digest should fail");

        assert!(error
            .to_string()
            .contains("Ready update trusted DMG verification is missing a digest"));
        assert_eq!(state.status, UpdateStatus::Failed);
        Ok(())
    }

    #[tokio::test]
    async fn install_ready_marks_missing_artifact_failed() -> Result<()> {
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
            auto_install_on_app_exit: false,
            notifications: false,
            developer_mode: false,
            workspace_root: temp.path().join("cache"),
            builder_bundle_root: temp.path().join("builder"),
            app_executable_path: temp.path().join("not-running-electron"),
            cli_path: None,
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(false);
        state.status = UpdateStatus::ReadyToInstall;
        state.candidate_version = Some("2999.03.25.010203+deadbeef".to_string());
        state.artifact_paths.package_path = Some(temp.path().join("missing/codex.deb"));

        let result = run_install_ready(&config, &mut state, &paths).await;

        assert!(result.is_err());
        assert_eq!(state.status, UpdateStatus::Failed);
        assert!(state
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains("Pending package artifact is missing")));
        Ok(())
    }

    #[tokio::test]
    async fn install_ready_marks_missing_package_record_failed() -> Result<()> {
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
            auto_install_on_app_exit: false,
            notifications: false,
            developer_mode: false,
            workspace_root: temp.path().join("cache"),
            builder_bundle_root: temp.path().join("builder"),
            app_executable_path: temp.path().join("not-running-electron"),
            cli_path: None,
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(false);
        state.status = UpdateStatus::ReadyToInstall;
        state.candidate_version = Some("2026.03.25.010203+deadbeef".to_string());

        let result = run_install_ready(&config, &mut state, &paths).await;

        assert!(result.is_err());
        assert_eq!(state.status, UpdateStatus::Failed);
        assert_eq!(
            state.error_message.as_deref(),
            Some("No ready update package is recorded")
        );
        Ok(())
    }

    #[tokio::test]
    async fn install_ready_reports_unrecoverable_interrupted_install() -> Result<()> {
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
            auto_install_on_app_exit: false,
            notifications: false,
            developer_mode: false,
            workspace_root: temp.path().join("cache"),
            builder_bundle_root: temp.path().join("builder"),
            app_executable_path: temp.path().join("not-running-electron"),
            cli_path: None,
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(false);
        state.status = UpdateStatus::Installing;
        state.candidate_version = Some("2026.03.25.010203+deadbeef".to_string());
        state.artifact_paths.package_path = Some(temp.path().join("missing/codex.deb"));

        let result = run_install_ready(&config, &mut state, &paths).await;

        assert!(result.is_err());
        assert_eq!(state.status, UpdateStatus::Failed);
        assert!(state
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains("interrupted")));
        Ok(())
    }

    #[test]
    fn pkexec_authentication_failures_are_retryable() -> Result<()> {
        for code in [126, 127] {
            let status = std::process::Command::new("/bin/sh")
                .arg("-c")
                .arg(format!("exit {code}"))
                .status()?;
            assert!(pkexec_authentication_was_not_obtained(&status));
        }

        let status = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("exit 1")
            .status()?;
        assert!(!pkexec_authentication_was_not_obtained(&status));
        Ok(())
    }

    #[test]
    fn command_output_summary_redacts_privileged_install_output() {
        let output = b"line one\nerror token=install-secret\nAuthorization: Bearer header-secret\n";

        let summary = summarize_command_output(output).expect("summary");

        assert_eq!(
            summary,
            "line one | error token=[REDACTED] | Authorization: [REDACTED]"
        );
        assert!(!summary.contains("install-secret"));
        assert!(!summary.contains("header-secret"));
    }

    #[test]
    fn command_lookup_requires_executable_file() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let candidate = temp.path().join("zenity");
        std::fs::write(&candidate, b"#!/bin/sh\n")?;

        let mut permissions = std::fs::metadata(&candidate)?.permissions();
        permissions.set_mode(0o644);
        std::fs::set_permissions(&candidate, permissions)?;

        assert!(!is_executable_file(&candidate));

        let mut permissions = std::fs::metadata(&candidate)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&candidate, permissions)?;

        assert!(is_executable_file(&candidate));
        Ok(())
    }

    #[test]
    fn prompt_install_cli_does_not_treat_non_executable_file_as_installed() -> Result<()> {
        let _env_guard = crate::test_util::env_lock();
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

        let _display_guard = crate::test_util::EnvVarGuard::remove(&_env_guard, "DISPLAY");
        let _wayland_display_guard =
            crate::test_util::EnvVarGuard::remove(&_env_guard, "WAYLAND_DISPLAY");
        let _dbus_session_bus_address_guard =
            crate::test_util::EnvVarGuard::remove(&_env_guard, "DBUS_SESSION_BUS_ADDRESS");
        let _xdg_runtime_dir_guard =
            crate::test_util::EnvVarGuard::remove(&_env_guard, "XDG_RUNTIME_DIR");
        let _path_guard = crate::test_util::EnvVarGuard::set(
            &_env_guard,
            "PATH",
            temp.path().join("missing-bin"),
        );
        let _home_guard = crate::test_util::EnvVarGuard::set(&_env_guard, "HOME", temp.path());
        let _nvm_dir_guard = crate::test_util::EnvVarGuard::remove(&_env_guard, "NVM_DIR");
        let _skip_system_cli_lookup_guard = crate::test_util::EnvVarGuard::set(
            &_env_guard,
            "CODEX_APP_UPDATER_TEST_SKIP_SYSTEM_CLI_LOOKUP",
            "1",
        );

        let invalid_cli_path = temp.path().join("codex.txt");
        std::fs::write(&invalid_cli_path, b"not executable")?;

        let mut state = PersistedState::new(true);
        state.cli_path = Some(invalid_cli_path);
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
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let outcome = prompt_install_cli(&config, &mut state, &paths, None)?;

        assert_eq!(outcome, PromptInstallCliOutcome::NoBackend);
        Ok(())
    }

    #[test]
    fn install_auth_retry_block_is_scoped_to_candidate() {
        let mut state = PersistedState::new(true);
        state.candidate_version = Some("2026.04.28.082247+abcdef12".to_string());

        assert!(!install_auth_retry_is_blocked(&state));

        state
            .notified_events
            .insert("install_auth_required:2026.04.28.082247+abcdef12".to_string());
        assert!(install_auth_retry_is_blocked(&state));

        state.candidate_version = Some("2026.04.29.010203+abcdef12".to_string());
        assert!(!install_auth_retry_is_blocked(&state));
    }

    #[test]
    fn clear_install_auth_required_event_keeps_unrelated_notifications() -> Result<()> {
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
        state.candidate_version = Some("2026.04.28.082247+abcdef12".to_string());
        state
            .notified_events
            .insert("install_auth_required:2026.04.28.082247+abcdef12".to_string());
        state
            .notified_events
            .insert("installed:2026.04.25.054929+12345678".to_string());

        clear_install_auth_required_event(&mut state, &paths)?;

        assert!(!install_auth_retry_is_blocked(&state));
        assert!(state
            .notified_events
            .contains("installed:2026.04.25.054929+12345678"));
        Ok(())
    }

    #[test]
    fn pending_install_becomes_installed_when_candidate_is_already_present() -> Result<()> {
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
        state.status = UpdateStatus::ReadyToInstall;
        state.installed_version = "2026.04.28.082247-abcdef12.fc43".to_string();
        state.candidate_version = Some("2026.04.28.082247+abcdef12".to_string());
        state.error_message = Some("authentication was not obtained".to_string());
        state
            .notified_events
            .insert("install_auth_required:2026.04.28.082247+abcdef12".to_string());

        assert!(complete_pending_install_if_already_installed(
            &paths.cache_dir,
            &mut state,
            &paths
        )?);

        assert_eq!(state.status, UpdateStatus::Installed);
        assert_eq!(state.candidate_version, None);
        assert_eq!(state.error_message, None);
        assert!(state.notified_events.is_empty());
        Ok(())
    }

    #[test]
    fn verified_same_version_pending_install_is_not_auto_completed() -> Result<()> {
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
        state.status = UpdateStatus::ReadyToInstall;
        state.installed_version = "26.513.31313".to_string();
        state.candidate_version = Some("26.513.31313".to_string());
        state.dmg_sha256 = Some(TRUSTED_TEST_DMG_SHA256.to_string());
        let package_path = temp.path().join("dist/codex.pkg.tar.zst");
        std::fs::create_dir_all(
            package_path
                .parent()
                .expect("package path should have parent"),
        )?;
        std::fs::write(&package_path, b"pkg")?;
        state.artifact_paths.package_path = Some(package_path);
        state.dmg_verification = Some(DmgVerification {
            result: DmgVerificationResult::Verified,
            version: Some("26.513.31313".to_string()),
            sha256: Some(TRUSTED_TEST_DMG_SHA256.to_string()),
            manifest_path: Some(temp.path().join("trusted-dmg-manifest.json")),
            verified_at: Some(Utc::now()),
            message: Some("Downloaded DMG matched repo-trusted metadata".to_string()),
        });

        assert!(!complete_pending_install_if_already_installed(
            &paths.cache_dir,
            &mut state,
            &paths
        )?);

        assert_eq!(state.status, UpdateStatus::ReadyToInstall);
        assert_eq!(state.candidate_version.as_deref(), Some("26.513.31313"));
        Ok(())
    }

    #[test]
    fn verified_current_dmg_requires_matching_candidate_version() {
        let mut state = PersistedState::new(true);
        state.candidate_version = Some("26.513.31314".to_string());
        state.dmg_sha256 = Some(TRUSTED_TEST_DMG_SHA256.to_string());
        state.dmg_verification = Some(DmgVerification {
            result: DmgVerificationResult::Verified,
            version: Some("26.513.31313".to_string()),
            sha256: Some(TRUSTED_TEST_DMG_SHA256.to_string()),
            manifest_path: Some(PathBuf::from("trusted-dmg-manifest.json")),
            verified_at: Some(Utc::now()),
            message: Some("Downloaded DMG matched repo-trusted metadata".to_string()),
        });

        assert!(!state_has_verified_current_dmg(&state));
    }

    #[test]
    fn pending_install_is_cleared_when_installed_version_is_newer() -> Result<()> {
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
        state.status = UpdateStatus::ReadyToInstall;
        state.installed_version = "2026.05.01.010203-99999999.fc43".to_string();
        state.candidate_version = Some("2026.04.28.082247+abcdef12".to_string());
        state.error_message = Some("authentication was not obtained".to_string());
        let superseded_package_path = temp.path().join("superseded.deb");
        std::fs::write(&superseded_package_path, b"deb")?;
        state.artifact_paths.package_path = Some(superseded_package_path);
        let workspace_root = temp.path().join("custom-workspace-root");
        state.artifact_paths.workspace_dir =
            Some(workspace_root.join("workspaces/2026.04.28.082247+abcdef12"));

        assert!(complete_pending_install_if_already_installed(
            &workspace_root,
            &mut state,
            &paths
        )?);

        assert_eq!(state.status, UpdateStatus::Installed);
        assert_eq!(state.candidate_version, None);
        assert_eq!(state.artifact_paths.package_path, None);
        assert_eq!(state.artifact_paths.workspace_dir, None);
        assert_eq!(state.error_message, None);
        crate::rollback::record_current_package_as_known_good(&mut state);
        assert_eq!(state.artifact_paths.rollback_package_path, None);
        Ok(())
    }

    fn status_clears_superseded_ready_update() -> Result<()> {
        let _env_guard = crate::test_util::env_lock();
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
        state.status = UpdateStatus::ReadyToInstall;
        state.installed_version = "2026.05.01.010203".to_string();
        state.candidate_version = Some("2026.04.28.082247+abcdef12".to_string());
        let superseded_package_path = temp.path().join("superseded-status.deb");
        std::fs::write(&superseded_package_path, b"deb")?;
        state.artifact_paths.package_path = Some(superseded_package_path);
        state.artifact_paths.workspace_dir = Some(
            temp.path()
                .join("cache/workspaces/2026.04.28.082247+abcdef12"),
        );

        let original_home = std::env::var_os("HOME");
        let original_path = std::env::var_os("PATH");
        let original_nvm_dir = std::env::var_os("NVM_DIR");
        let original_codex_cli_path = std::env::var_os("CODEX_CLI_PATH");
        let original_skip_system_cli_lookup =
            std::env::var_os("CODEX_UPDATE_MANAGER_SKIP_SYSTEM_CLI_LOOKUP");
        std::env::set_var("HOME", temp.path());
        std::env::set_var("PATH", temp.path().join("missing-bin"));
        std::env::remove_var("NVM_DIR");
        std::env::remove_var("CODEX_CLI_PATH");
        std::env::set_var("CODEX_UPDATE_MANAGER_SKIP_SYSTEM_CLI_LOOKUP", "1");

        let config = test_config(temp.path());
        let result = run_status(&config, &mut state, &paths, true);

        if let Some(home) = original_home {
            std::env::set_var("HOME", home);
        } else {
            std::env::remove_var("HOME");
        }
        if let Some(path) = original_path {
            std::env::set_var("PATH", path);
        } else {
            std::env::remove_var("PATH");
        }
        if let Some(nvm_dir) = original_nvm_dir {
            std::env::set_var("NVM_DIR", nvm_dir);
        } else {
            std::env::remove_var("NVM_DIR");
        }
        if let Some(cli_path) = original_codex_cli_path {
            std::env::set_var("CODEX_CLI_PATH", cli_path);
        } else {
            std::env::remove_var("CODEX_CLI_PATH");
        }
        if let Some(value) = original_skip_system_cli_lookup {
            std::env::set_var("CODEX_UPDATE_MANAGER_SKIP_SYSTEM_CLI_LOOKUP", value);
        } else {
            std::env::remove_var("CODEX_UPDATE_MANAGER_SKIP_SYSTEM_CLI_LOOKUP");
        }

        result?;

        assert_eq!(state.status, UpdateStatus::Installed);
        assert_eq!(state.candidate_version, None);
        assert_eq!(state.artifact_paths.package_path, None);
        assert_eq!(state.artifact_paths.workspace_dir, None);
        Ok(())
    }

    #[test]
    fn status_reports_cli_update_required() -> Result<()> {
        let _env_guard = crate::test_util::env_lock();
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

        let bin_dir = temp.path().join("bin");
        fs::create_dir_all(&bin_dir)?;
        let codex_path = bin_dir.join("codex");
        fs::write(
            &codex_path,
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ] || [ \"$1\" = \"version\" ]; then\n  echo 'codex-cli v0.42.0'\n  exit 0\nfi\nexit 1\n",
        )?;
        let mut permissions = fs::metadata(&codex_path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&codex_path, permissions)?;

        let npm_path = bin_dir.join("npm");
        fs::write(
            &npm_path,
            "#!/bin/sh\nif [ \"$1\" = \"view\" ] && [ \"$2\" = \"@openai/codex\" ] && [ \"$3\" = \"version\" ]; then\n  echo '0.42.1'\n  exit 0\nfi\nexit 1\n",
        )?;
        let mut permissions = fs::metadata(&npm_path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&npm_path, permissions)?;

        let original_home = std::env::var_os("HOME");
        let original_path = std::env::var_os("PATH");
        let original_nvm_dir = std::env::var_os("NVM_DIR");
        let original_codex_cli_path = std::env::var_os("CODEX_CLI_PATH");
        let original_skip_system_cli_lookup =
            std::env::var_os("CODEX_UPDATE_MANAGER_SKIP_SYSTEM_CLI_LOOKUP");
        std::env::set_var("HOME", temp.path());
        std::env::set_var("PATH", std::env::join_paths([bin_dir])?);
        std::env::remove_var("NVM_DIR");
        std::env::remove_var("CODEX_CLI_PATH");
        std::env::set_var("CODEX_UPDATE_MANAGER_SKIP_SYSTEM_CLI_LOOKUP", "1");

        let config = test_config(temp.path());
        let mut state = PersistedState::new(true);
        state.cli_path = Some(codex_path);
        let result = run_status(&config, &mut state, &paths, true);

        if let Some(home) = original_home {
            std::env::set_var("HOME", home);
        } else {
            std::env::remove_var("HOME");
        }
        if let Some(path) = original_path {
            std::env::set_var("PATH", path);
        } else {
            std::env::remove_var("PATH");
        }
        if let Some(nvm_dir) = original_nvm_dir {
            std::env::set_var("NVM_DIR", nvm_dir);
        } else {
            std::env::remove_var("NVM_DIR");
        }
        if let Some(cli_path) = original_codex_cli_path {
            std::env::set_var("CODEX_CLI_PATH", cli_path);
        } else {
            std::env::remove_var("CODEX_CLI_PATH");
        }
        if let Some(value) = original_skip_system_cli_lookup {
            std::env::set_var("CODEX_UPDATE_MANAGER_SKIP_SYSTEM_CLI_LOOKUP", value);
        } else {
            std::env::remove_var("CODEX_UPDATE_MANAGER_SKIP_SYSTEM_CLI_LOOKUP");
        }

        result?;
        assert_eq!(state.cli_status, CliStatus::UpdateRequired);
        assert_eq!(state.cli_installed_version.as_deref(), Some("0.42.0"));
        assert_eq!(state.cli_latest_version.as_deref(), Some("0.42.1"));
        Ok(())
    }

    #[test]
    fn generated_versions_compare_by_timestamp_segments() {
        assert_eq!(
            compare_generated_versions("2026.04.01.035152", "2026.03.27.025604+1086e799"),
            Some(std::cmp::Ordering::Greater)
        );
    }

    #[test]
    fn generated_versions_ignore_package_release_suffixes() {
        assert_eq!(
            compare_generated_versions(
                "2026.04.25.054929-90dd7716x11.fc43",
                "2026.04.25.054929+90dd7716",
            ),
            Some(std::cmp::Ordering::Equal)
        );
    }

    #[test]
    fn generated_version_comparison_supports_dmg_app_versions() {
        assert_eq!(
            compare_generated_versions("26.429.20946", "26.428.10000"),
            Some(std::cmp::Ordering::Greater)
        );
    }

    #[test]
    fn rollback_block_prefers_stable_dmg_hash() {
        let mut state = PersistedState::new(true);
        state.rollback_blocked_dmg_sha256 = Some("badcafe0".repeat(8));

        assert!(downloaded_dmg_is_blocked_by_rollback(
            &state,
            "26.513.31313",
            "2026.05.07.091500+fresh123",
            &"badcafe0".repeat(8),
        ));
        assert!(!downloaded_dmg_is_blocked_by_rollback(
            &state,
            "26.513.31313",
            "2026.05.07.091500+fresh123",
            &"feedface".repeat(8),
        ));
    }

    #[test]
    fn rollback_block_keeps_candidate_version_fallback() {
        let mut state = PersistedState::new(true);
        state.rollback_blocked_candidate_version = Some("2026.05.06.120000+badcafe0".to_string());

        assert!(downloaded_dmg_is_blocked_by_rollback(
            &state,
            "26.513.31313",
            "2026.05.06.120000+fresh123",
            &"feedface".repeat(8),
        ));
    }

    #[test]
    fn rollback_block_keeps_legacy_download_candidate_version_fallback() {
        let mut state = PersistedState::new(true);
        state.rollback_blocked_candidate_version = Some("2026.05.06.120000+badcafe0".to_string());

        assert!(downloaded_dmg_is_blocked_by_rollback(
            &state,
            "26.513.31313",
            "2026.05.06.120000+fresh123",
            &"feedface".repeat(8),
        ));
    }

    #[test]
    fn rollback_block_does_not_compare_legacy_timestamp_to_trusted_app_version() {
        let mut state = PersistedState::new(true);
        state.rollback_blocked_candidate_version = Some("2026.05.06.120000+badcafe0".to_string());

        assert!(!downloaded_dmg_is_blocked_by_rollback(
            &state,
            "26.513.31313",
            "2026.05.07.120000+fresh123",
            &"feedface".repeat(8),
        ));
    }

    #[test]
    fn rollback_block_does_not_block_same_app_version_with_different_digest() {
        let mut state = PersistedState::new(true);
        state.rollback_blocked_candidate_version = Some("26.513.31313".to_string());
        state.rollback_blocked_dmg_sha256 = Some("badcafe0".repeat(8));

        assert!(!downloaded_dmg_is_blocked_by_rollback(
            &state,
            "26.513.31313",
            "2026.05.07.120000+fresh123",
            &"feedface".repeat(8),
        ));
        assert!(downloaded_dmg_is_blocked_by_rollback(
            &state,
            "26.513.31313",
            "2026.05.07.120000+fresh123",
            &"badcafe0".repeat(8),
        ));
    }

    #[tokio::test]
    async fn trusted_dmg_recheck_fails_when_cached_file_changes() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let dmg_path = temp.path().join("Codex.dmg");
        tokio::fs::write(&dmg_path, b"trusted bytes").await?;
        let expected_sha256 = file_sha256(&dmg_path).await?;
        let verified = trust::VerifiedDmg {
            version: "26.513.31313".to_string(),
            sha256: expected_sha256,
            manifest_path: temp.path().join("trusted-dmg-manifest.json"),
        };

        ensure_downloaded_dmg_still_matches_verified_metadata(&dmg_path, &verified).await?;

        tokio::fs::write(&dmg_path, b"changed bytes").await?;
        let error = ensure_downloaded_dmg_still_matches_verified_metadata(&dmg_path, &verified)
            .await
            .expect_err("changed cached DMG should fail the rebuild-time hash check");
        assert!(error
            .to_string()
            .contains("Downloaded DMG changed after trusted metadata verification"));
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
        state.installed_version = "2026.04.01.035152".to_string();
        state.candidate_version = Some("2026.03.27.025604+1086e799".to_string());
        state.artifact_paths.package_path = Some(package_path);
        let recovered_workspace = temp
            .path()
            .join("cache/workspaces/2026.03.27.025604+1086e799");
        std::fs::create_dir_all(&recovered_workspace)?;
        state.artifact_paths.workspace_dir = Some(recovered_workspace.clone());

        recover_interrupted_install(&paths.cache_dir, &mut state, &paths)?;

        assert_eq!(state.status, UpdateStatus::Installed);
        assert_eq!(state.candidate_version, None);
        assert_eq!(state.artifact_paths.package_path, None);
        assert_eq!(state.artifact_paths.workspace_dir, None);
        assert_eq!(state.error_message, None);
        assert!(!recovered_workspace.exists());
        Ok(())
    }

    #[test]
    fn status_recovers_interrupted_install_before_reporting() -> Result<()> {
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

        let codex_cli_path = temp.path().join("codex-cli");
        std::fs::write(
            &codex_cli_path,
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'codex-cli v0.42.0'\n  exit 0\nfi\nexit 1\n",
        )?;
        let mut cli_permissions = std::fs::metadata(&codex_cli_path)?.permissions();
        cli_permissions.set_mode(0o755);
        std::fs::set_permissions(&codex_cli_path, cli_permissions)?;

        let config = RuntimeConfig {
            dmg_url: "https://example.com/Codex.dmg".to_string(),
            initial_check_delay_seconds: 1,
            check_interval_hours: 6,
            auto_install_on_app_exit: false,
            notifications: false,
            developer_mode: false,
            workspace_root: paths.cache_dir.clone(),
            builder_bundle_root: temp.path().join("builder"),
            app_executable_path: temp.path().join("codex-app"),
            cli_path: Some(codex_cli_path),
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        let mut state = PersistedState::new(false);
        state.status = UpdateStatus::Installing;
        state.installed_version = "2026.03.24.120000".to_string();
        state.candidate_version = Some("2026.03.27.025604+1086e799".to_string());
        state.artifact_paths.package_path = Some(temp.path().join("dist/missing-codex.deb"));

        run_status(&config, &mut state, &paths, true)?;

        assert_eq!(state.status, UpdateStatus::Failed);
        assert!(state
            .error_message
            .as_deref()
            .is_some_and(|message| message.contains("package artifact is missing")));
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
        state.installed_version = "2026.03.24.120000".to_string();
        state.candidate_version = Some("2026.03.27.025604+1086e799".to_string());
        state.artifact_paths.package_path = Some(package_path);

        recover_interrupted_install(&paths.cache_dir, &mut state, &paths)?;

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
        state.candidate_version = Some("2026.03.24+abcd1234".to_string());
        maybe_notify(
            &mut state,
            &paths,
            false,
            "ready_to_install",
            "Codex App update ready",
            "An update is ready to install.",
        )?;
        let notified_count = state.notified_events.len();
        maybe_notify(
            &mut state,
            &paths,
            false,
            "ready_to_install",
            "Codex App update ready",
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
        state.installed_version = "2026.04.16.120000".to_string();

        maybe_notify_installed(&mut state, &paths, false)?;
        let notified_count = state.notified_events.len();
        maybe_notify_installed(&mut state, &paths, false)?;

        assert_eq!(state.notified_events.len(), notified_count);
        assert!(state
            .notified_events
            .contains("installed:2026.04.16.120000"));
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
        state.cli_status = CliStatus::NotInstalled;
        state.cli_error_message = Some(codex_cli::CLI_NOT_INSTALLED_MESSAGE.to_string());

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
