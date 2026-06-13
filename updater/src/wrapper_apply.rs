//! Applies a pending wrapper (repo) update for the current install type.
//!
//! Invoked by the optional `codex-wrapper-updater` port integration when it sees a
//! pending apply marker. Detection (see [`crate::wrapper`]) only records that a
//! newer wrapper build exists; this module performs the actual rebuild + install:
//!
//! - **User-local** installs reuse `~/.local/bin/codex-app-update`, which
//!   pulls the managed checkout and re-runs `install.sh` in place as the user
//!   (no privilege escalation).
//! - **Packaged** installs fetch the wrapper source into a managed clone, build
//!   a fresh native package from the cached DMG, and install it with `pkexec`.
//!   When the build toolchain (cargo / node / a DMG extractor) is missing, this
//!   sends a desktop notification and returns an error so the integration marker can
//!   remain in place for a later retry.

use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    collections::HashSet,
    fs,
    os::unix::{
        fs::{self as unix_fs, PermissionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tracing::{info, warn};

use crate::{
    builder,
    config::{RuntimeConfig, RuntimePaths},
    dmg_source, install, notify, package_verification,
    state::{DmgVerification, DmgVerificationResult, PersistedState, UpdateStatus},
    trust, wrapper,
};

#[cfg(test)]
const GIT_COMMAND_TIMEOUT: Duration = Duration::from_millis(200);
#[cfg(not(test))]
const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(20);
const GIT_POLL_INTERVAL: Duration = Duration::from_millis(50);
const SIGKILL: i32 = 9;

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

/// How the running app was installed, which determines how a wrapper update is
/// applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallType {
    /// Native package under `/opt/codex-app` with a system package record.
    Packaged,
    /// `install.sh` install under the user's home (`~/.local/...`).
    UserLocal,
}

fn detect_install_type(config: &RuntimeConfig) -> InstallType {
    // The launcher knows which install is actually running and exports its app
    // directory. Prefer that authoritative hint: an app dir under /opt is the
    // packaged install; anything else (e.g. ~/.local/opt) is user-local. This
    // disambiguates machines that have both a .deb and a user-local install.
    if let Some(app_dir) = std::env::var_os("CODEX_LINUX_APP_DIR") {
        let app_dir = PathBuf::from(app_dir);
        if app_dir.starts_with("/opt/") {
            return InstallType::Packaged;
        }
        return InstallType::UserLocal;
    }

    // Fallback when no launcher hint is present: a packaged builder bundle plus
    // an installed system package indicates the packaged install.
    let packaged_bundle = Path::new("/usr/lib/codex-app/update-builder");
    if config.builder_bundle_root == packaged_bundle && install::is_primary_package_installed() {
        InstallType::Packaged
    } else {
        InstallType::UserLocal
    }
}

/// Applies a pending wrapper update. No-ops when wrapper tracking is disabled.
pub async fn run_apply_wrapper_update(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<()> {
    if !config.enable_wrapper_updates {
        println!("Wrapper update tracking is disabled; nothing to apply.");
        return Ok(());
    }

    if state.wrapper_dev_mode == Some(true) {
        warn!("wrapper apply refused because installed wrapper appears ahead of upstream");
        println!("Wrapper is a local/dev build ahead of upstream; not applying (would downgrade).");
        return Ok(());
    }

    if state.candidate_wrapper_commit.as_deref().is_none() {
        println!("No wrapper update candidate is ready; nothing to apply.");
        return Ok(());
    }

    let candidate_commit = state.candidate_wrapper_commit.clone();
    let result = match detect_install_type(config) {
        InstallType::UserLocal => {
            apply_user_local(config, paths, candidate_commit.as_deref()).await
        }
        InstallType::Packaged => {
            apply_packaged(config, state, paths, candidate_commit.as_deref()).await
        }
    };

    match result {
        Ok(()) => {
            state.installed_version = install::installed_package_version();
            state.candidate_version = None;
            state.status = UpdateStatus::Installed;
            state.error_message = None;
            state.notified_events.clear();
            state.artifact_paths.workspace_dir = None;
            state.artifact_paths.package_path = None;
            refresh_installed_wrapper_state(config, state);
            state.clear_wrapper_update_candidate();
            state.save(&paths.state_file)?;
            let _ = notify::send(
                "Codex App updated",
                "The newer Linux wrapper build has been installed.",
            );
            Ok(())
        }
        Err(error) => {
            warn!(?error, "wrapper update apply failed");
            Err(error)
        }
    }
}

fn refresh_installed_wrapper_state(config: &RuntimeConfig, state: &mut PersistedState) {
    if let Some(installed) = wrapper::installed_wrapper_from_metadata(
        &config.app_executable_path,
        &config.builder_bundle_root,
    ) {
        state.installed_wrapper_version = installed.version;
        state.installed_wrapper_commit = Some(installed.commit);
    }
}

/// User-local apply. Prefers the contrib `codex-app-update` helper (managed
/// checkout pull + in-place `install.sh`) when present; otherwise falls back to
/// fetching the wrapper source and running its `install.sh` directly against the
/// running app dir. Runs as the user, no privilege escalation.
async fn apply_user_local(
    config: &RuntimeConfig,
    paths: &RuntimePaths,
    candidate_commit: Option<&str>,
) -> Result<()> {
    let integration_config = effective_integration_config(config);
    if let Some(helper) = user_local_update_helper() {
        info!(helper = %helper.display(), "applying wrapper update via user-local helper");
        let mut cmd = Command::new(&helper);
        cmd.arg("--quiet");
        if let Some(app_dir) = user_local_app_dir() {
            if let Some(install_root) = app_dir.parent() {
                cmd.env("CODEX_USER_INSTALL_ROOT", install_root);
            }
        }
        // The contrib helper honors a caller-set CODEX_PORT_INTEGRATIONS_CONFIG over
        // its repo-local default, so the in-app picker's selection wins.
        if let Some(config_path) = &integration_config {
            cmd.env("CODEX_PORT_INTEGRATIONS_CONFIG", config_path);
            cmd.env("CODEX_LINUX_FEATURES_CONFIG", config_path);
        }
        let status = cmd
            .status()
            .with_context(|| format!("Failed to run {}", helper.display()))?;
        if !status.success() {
            anyhow::bail!("{} exited with status {status}", helper.display());
        }
        return Ok(());
    }

    // Fallback: rebuild in place from a freshly fetched wrapper source.
    let app_dir = user_local_app_dir()
        .context("could not resolve user-local app dir (CODEX_LINUX_APP_DIR)")?;
    let install_root = app_dir
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| app_dir.clone());
    let wrapper_src = ensure_wrapper_source(config, paths, candidate_commit)?;
    stage_enabled_local_integrations(config, &wrapper_src, integration_config.as_deref())?;
    let install_sh = wrapper_src.join("install.sh");
    if !install_sh.is_file() {
        anyhow::bail!(
            "wrapper source is missing install.sh at {}",
            install_sh.display()
        );
    }
    info!(app_dir = %app_dir.display(), "rebuilding user-local app in place via install.sh");
    let mut cmd = Command::new(&install_sh);
    cmd.current_dir(&wrapper_src)
        .env("CODEX_INSTALL_ALLOW_RUNNING", "1")
        .env("CODEX_INSTALL_ROOT", &install_root)
        .env("CODEX_INSTALL_DIR", &app_dir);
    if let Some(config_path) = &integration_config {
        cmd.env("CODEX_PORT_INTEGRATIONS_CONFIG", config_path);
        cmd.env("CODEX_LINUX_FEATURES_CONFIG", config_path);
    }
    let status = cmd
        .status()
        .with_context(|| format!("Failed to run {}", install_sh.display()))?;
    if !status.success() {
        anyhow::bail!("{} exited with status {status}", install_sh.display());
    }
    Ok(())
}

/// The integration selection to use for this rebuild: saved picker selection first,
/// then the installed builder bundle's preserved integration config.
fn effective_integration_config(config: &RuntimeConfig) -> Option<PathBuf> {
    crate::config::effective_integration_config_path(config)
}

fn valid_integration_id(id: &str) -> bool {
    let mut bytes = id.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn enabled_integration_ids_from_config(config_path: &Path) -> Vec<String> {
    let content = match fs::read_to_string(config_path) {
        Ok(content) => content,
        Err(error) => {
            warn!(path = %config_path.display(), error = %error, "could not read port integration config");
            return Vec::new();
        }
    };
    let value = match serde_json::from_str::<Value>(&content) {
        Ok(value) => value,
        Err(error) => {
            warn!(path = %config_path.display(), error = %error, "could not parse port integration config");
            return Vec::new();
        }
    };
    let Some(enabled) = value.get("enabled").and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut seen = HashSet::new();
    let mut ids = Vec::new();
    for item in enabled {
        let Some(id) = item.as_str() else {
            continue;
        };
        if !valid_integration_id(id) || !seen.insert(id.to_string()) {
            continue;
        }
        ids.push(id.to_string());
    }
    ids
}

fn copy_dir_all(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target).with_context(|| format!("Failed to create {}", target.display()))?;
    for entry in
        fs::read_dir(source).with_context(|| format!("Failed to read {}", source.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path)
            .with_context(|| format!("Failed to stat {}", source_path.display()))?;
        let file_type = metadata.file_type();
        if file_type.is_dir() {
            copy_dir_all(&source_path, &target_path)?;
        } else if file_type.is_symlink() {
            let link_target = fs::read_link(&source_path)
                .with_context(|| format!("Failed to read symlink {}", source_path.display()))?;
            unix_fs::symlink(&link_target, &target_path).with_context(|| {
                format!(
                    "Failed to copy symlink {} to {}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &target_path).with_context(|| {
                format!(
                    "Failed to copy {} to {}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
            fs::set_permissions(&target_path, metadata.permissions()).with_context(|| {
                format!("Failed to set permissions on {}", target_path.display())
            })?;
        }
    }
    Ok(())
}

fn stage_enabled_local_integrations(
    config: &RuntimeConfig,
    wrapper_src: &Path,
    integration_config: Option<&Path>,
) -> Result<()> {
    let Some(integration_config) = integration_config else {
        return Ok(());
    };
    if !integration_config.is_file() {
        return Ok(());
    }

    let source_local_root = config.builder_bundle_root.join("port-integrations/local");
    if !source_local_root.is_dir() {
        return Ok(());
    }

    let target_integrations_root = wrapper_src.join("port-integrations");
    for id in enabled_integration_ids_from_config(integration_config) {
        let source_dir = source_local_root.join(&id);
        if !source_dir.join("integration.json").is_file() {
            continue;
        }

        // If the fetched wrapper gained a real top-level integration with this
        // id, prefer the upstream integration and avoid creating a duplicate.
        if target_integrations_root
            .join(&id)
            .join("integration.json")
            .is_file()
        {
            continue;
        }

        let target_dir = target_integrations_root.join("local").join(&id);
        if target_dir.exists() {
            fs::remove_dir_all(&target_dir)
                .with_context(|| format!("Failed to remove {}", target_dir.display()))?;
        }
        copy_dir_all(&source_dir, &target_dir)?;
    }
    Ok(())
}

fn user_local_update_helper() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let candidate = home.join(".local/bin/codex-app-update");
    if candidate.is_file()
        && candidate
            .metadata()
            .is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
    {
        Some(candidate)
    } else {
        None
    }
}

/// The running user-local app directory, from the launcher's `CODEX_LINUX_APP_DIR`.
fn user_local_app_dir() -> Option<PathBuf> {
    std::env::var_os("CODEX_LINUX_APP_DIR")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

/// Packaged apply: fetch fresh wrapper source, rebuild a native package from the
/// cached DMG, and install it with pkexec.
async fn apply_packaged(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
    candidate_commit: Option<&str>,
) -> Result<()> {
    if let Some(missing) = missing_build_dependency() {
        let body = format!(
            "A newer Codex App build is available, but '{missing}' is needed to rebuild it. Install the build tools or update the package manually."
        );
        let _ = notify::send("Codex App update available", &body);
        println!("{body}");
        anyhow::bail!("missing build dependency for wrapper update: {missing}");
    }

    let wrapper_src = ensure_wrapper_source(config, paths, candidate_commit)?;
    seed_packaged_builder_payload(config, &wrapper_src)?;
    let integration_config = effective_integration_config(config);
    stage_enabled_local_integrations(config, &wrapper_src, integration_config.as_deref())?;
    let dmg_path = cached_or_downloaded_dmg(config, state, paths).await?;

    // Keep wrapper rebuild workspaces unique even when the official app version
    // is unchanged; the produced package still uses the official app version.
    let workspace_version = derive_workspace_version(&dmg_path)?;

    let artifacts = builder::build_update_from(
        &wrapper_src,
        config,
        state,
        paths,
        &workspace_version,
        &dmg_path,
    )
    .await
    .context("wrapper package rebuild failed")?;

    let package_candidate_version = state
        .candidate_version
        .as_deref()
        .context("wrapper rebuild did not record a package candidate version")?;
    let expected_package = expected_package_for_wrapper_install(
        config,
        state,
        &artifacts.package_path,
        package_candidate_version,
    )?;
    let output = install::pkexec_command_with_options(
        &artifacts.package_path,
        Some(&expected_package),
        true,
    )?
    .output()
    .context("Failed to launch pkexec for wrapper update installation")?;
    if !output.status.success() {
        anyhow::bail!(
            "privileged wrapper install exited with status {}",
            output.status
        );
    }

    state.installed_version = install::installed_package_version();
    let _ = state.save(&paths.state_file);
    Ok(())
}

fn seed_packaged_builder_payload(config: &RuntimeConfig, wrapper_src: &Path) -> Result<()> {
    builder::seed_builder_only_payload(&config.builder_bundle_root, wrapper_src)
        .context("failed to seed generated builder payload for wrapper rebuild")
}

fn expected_package_for_wrapper_install(
    config: &RuntimeConfig,
    state: &PersistedState,
    package_path: &Path,
    candidate_version: &str,
) -> Result<install::ExpectedPackage> {
    package_verification::expected_package_for_ready_install(
        package_path,
        &config.workspace_root,
        Some(candidate_version),
        state.dmg_sha256.as_deref(),
        state.package_verification.as_ref(),
    )
}

/// Clones or refreshes a managed wrapper checkout under the workspace cache and
/// returns its path. Never touches the user's working tree.
pub(crate) fn ensure_wrapper_source(
    config: &RuntimeConfig,
    paths: &RuntimePaths,
    candidate_commit: Option<&str>,
) -> Result<PathBuf> {
    let remote = wrapper::resolve_remote(&config.wrapper_remote, &config.builder_bundle_root);
    let branch = if config.wrapper_branch.trim().is_empty() {
        "main"
    } else {
        config.wrapper_branch.trim()
    };
    let dest = paths.cache_dir.join("wrapper-src");

    if dest.join(".git").is_dir() {
        run_git(&[
            "-C",
            &dest.to_string_lossy(),
            "fetch",
            "--depth",
            "1",
            "--quiet",
            &remote,
            branch,
        ])?;
        if let Some(commit) = candidate_commit {
            run_git(&[
                "-C",
                &dest.to_string_lossy(),
                "fetch",
                "--depth",
                "1",
                "--quiet",
                &remote,
                commit,
            ])?;
        }
        run_git(&[
            "-C",
            &dest.to_string_lossy(),
            "reset",
            "--hard",
            "--quiet",
            candidate_commit.unwrap_or("FETCH_HEAD"),
        ])?;
        run_git(&["-C", &dest.to_string_lossy(), "clean", "-fdx", "--quiet"])?;
    } else {
        std::fs::create_dir_all(&paths.cache_dir)
            .with_context(|| format!("Failed to create {}", paths.cache_dir.display()))?;
        let _ = std::fs::remove_dir_all(&dest);
        run_git(&[
            "clone",
            "--depth",
            "1",
            "--branch",
            branch,
            "--single-branch",
            "--quiet",
            &remote,
            &dest.to_string_lossy(),
        ])?;
        if let Some(commit) = candidate_commit {
            run_git(&[
                "-C",
                &dest.to_string_lossy(),
                "fetch",
                "--depth",
                "1",
                "--quiet",
                &remote,
                commit,
            ])?;
            run_git(&[
                "-C",
                &dest.to_string_lossy(),
                "reset",
                "--hard",
                "--quiet",
                commit,
            ])?;
        }
    }

    Ok(dest)
}

fn guarded_git_ssh_command() -> String {
    let base = std::env::var("GIT_SSH_COMMAND")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "ssh".to_string());
    format!("{base} -oBatchMode=yes -oStrictHostKeyChecking=yes")
}

fn git_command(args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "true")
        .env("SSH_ASKPASS", "true")
        .env("GCM_INTERACTIVE", "never")
        .env("GIT_SSH_COMMAND", guarded_git_ssh_command());
    command.process_group(0);
    command
}

fn kill_child_process_group(child: &mut std::process::Child) {
    let pgid = child.id() as i32;
    // SAFETY: `git_command` starts git in its own process group, so a negative
    // pgid targets only the subprocess tree we created for this git operation.
    unsafe {
        let _ = kill(-pgid, SIGKILL);
    }
    let _ = child.kill();
}

fn run_git(args: &[&str]) -> Result<()> {
    let mut child = git_command(args)
        .spawn()
        .context("Failed to run git for wrapper source")?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .context("Failed to wait for git wrapper source command")?
        {
            break status;
        }
        if started.elapsed() >= GIT_COMMAND_TIMEOUT {
            kill_child_process_group(&mut child);
            let _ = child.wait();
            anyhow::bail!(
                "git {:?} timed out after {} seconds",
                args,
                GIT_COMMAND_TIMEOUT.as_secs_f64()
            );
        }
        thread::sleep(GIT_POLL_INTERVAL);
    };
    if !status.success() {
        anyhow::bail!("git {:?} exited with status {status}", args);
    }
    Ok(())
}

/// Returns the cached DMG path, downloading it if no usable cache exists.
async fn cached_or_downloaded_dmg(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<PathBuf> {
    if let Some(dmg) = state.artifact_paths.dmg_path.clone() {
        if dmg.exists() {
            trust_dmg_for_wrapper_rebuild(config, state, paths, &dmg, None)?;
            return Ok(dmg);
        }
    }

    let client = reqwest::Client::builder().build()?;
    let downloads_dir = config.workspace_root.join("downloads");
    let downloaded =
        dmg_source::download_dmg(&client, &config.dmg_url, &downloads_dir, chrono::Utc::now())
            .await
            .context("Failed to download official DMG for wrapper rebuild")?;
    trust_dmg_for_wrapper_rebuild(
        config,
        state,
        paths,
        &downloaded.path,
        Some(downloaded.sha256.as_str()),
    )?;
    state.artifact_paths.dmg_path = Some(downloaded.path.clone());
    let _ = state.save(&paths.state_file);
    Ok(downloaded.path)
}

fn trust_dmg_for_wrapper_rebuild(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
    dmg_path: &Path,
    known_sha256: Option<&str>,
) -> Result<()> {
    let dmg_sha256 = match known_sha256 {
        Some(value) => value.to_string(),
        None => package_verification::file_sha256(dmg_path)?,
    };
    let manifest_path = trust::trusted_dmg_manifest_path(&config.builder_bundle_root);
    match trust::verify_downloaded_dmg_with_manifest(&manifest_path, &config.dmg_url, &dmg_sha256) {
        Ok(verified) => {
            state.dmg_sha256 = Some(verified.sha256.clone());
            state.dmg_verification = Some(DmgVerification {
                result: DmgVerificationResult::Verified,
                version: Some(verified.version),
                sha256: Some(verified.sha256),
                manifest_path: Some(verified.manifest_path),
                verified_at: Some(chrono::Utc::now()),
                message: Some("Wrapper rebuild DMG matched repo-trusted metadata".to_string()),
            });
            state.artifact_paths.dmg_path = Some(dmg_path.to_path_buf());
            state.save(&paths.state_file)?;
            Ok(())
        }
        Err(error) => {
            state.dmg_verification = Some(DmgVerification {
                result: DmgVerificationResult::Failed,
                version: None,
                sha256: Some(dmg_sha256),
                manifest_path: Some(manifest_path),
                verified_at: Some(chrono::Utc::now()),
                message: Some(error.to_string()),
            });
            state.artifact_paths.dmg_path = Some(dmg_path.to_path_buf());
            let _ = state.save(&paths.state_file);
            Err(error)
        }
    }
}

/// Derives a monotonic workspace key (`YYYY.MM.DD.HHMMSS+<sha8>`) from the DMG
/// contents, matching the DMG update path's workspace naming scheme.
fn derive_workspace_version(dmg_path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(dmg_path)
        .with_context(|| format!("Failed to read {}", dmg_path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let sha = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    dmg_source::derive_candidate_version(&sha, chrono::Utc::now())
}

/// Returns the first missing build dependency needed for a packaged rebuild, or
/// `None` when the toolchain is present.
fn missing_build_dependency() -> Option<&'static str> {
    // install.sh needs a DMG extractor (7z/7zz) and the package build runs cargo
    // for the updater; node is provided by the bundled managed runtime.
    for (tool, label) in [("cargo", "cargo"), ("7zz", "7zz")] {
        if which(tool).is_none() {
            // 7z is an acceptable alternative to 7zz.
            if tool == "7zz" && which("7z").is_some() {
                continue;
            }
            return Some(label);
        }
    }
    None
}

fn which(tool: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(tool);
        if candidate.is_file()
            && candidate
                .metadata()
                .is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
        {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::PackageVerification;
    use crate::test_util::env_lock;
    use tempfile::tempdir;

    fn test_paths(root: &Path) -> RuntimePaths {
        RuntimePaths {
            config_file: root.join("config/config.toml"),
            state_file: root.join("state/state.json"),
            log_file: root.join("state/service.log"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            config_dir: root.join("config"),
        }
    }

    fn test_config(root: &Path) -> RuntimeConfig {
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
            enable_wrapper_updates: true,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        }
    }

    fn write_local_integration(root: &Path, id: &str) {
        let integration_dir = root.join("builder/port-integrations/local").join(id);
        std::fs::create_dir_all(integration_dir.join("nested")).unwrap();
        std::fs::write(
            integration_dir.join("integration.json"),
            format!(
                r#"{{
  "id": "{id}",
  "title": "Local Integration",
  "description": "Local test integration",
  "defaultEnabled": false,
  "entrypoints": {{}}
}}"#
            ),
        )
        .unwrap();
        std::fs::write(integration_dir.join("README.md"), "# Local Integration\n").unwrap();
        std::fs::write(integration_dir.join("nested/payload.txt"), "payload\n").unwrap();
        unix_fs::symlink("nested/payload.txt", integration_dir.join("payload-link")).unwrap();
    }

    #[test]
    fn stages_enabled_local_integrations_into_wrapper_source() {
        let root = tempdir().unwrap();
        let config = test_config(root.path());
        let wrapper_src = root.path().join("wrapper-src");
        let integration_config = root.path().join("port-integrations.json");
        write_local_integration(root.path(), "model-provider-switcher");
        std::fs::create_dir_all(wrapper_src.join("port-integrations")).unwrap();
        std::fs::write(
            &integration_config,
            r#"{"enabled":["agent-workspace","model-provider-switcher","missing-local"]}"#,
        )
        .unwrap();

        stage_enabled_local_integrations(&config, &wrapper_src, Some(&integration_config)).unwrap();

        assert!(wrapper_src
            .join("port-integrations/local/model-provider-switcher/integration.json")
            .is_file());
        assert_eq!(
            std::fs::read_to_string(
                wrapper_src
                    .join("port-integrations/local/model-provider-switcher/nested/payload.txt")
            )
            .unwrap(),
            "payload\n"
        );
        assert_eq!(
            std::fs::read_link(
                wrapper_src.join("port-integrations/local/model-provider-switcher/payload-link")
            )
            .unwrap(),
            PathBuf::from("nested/payload.txt")
        );
        assert!(!wrapper_src
            .join("port-integrations/local/missing-local/integration.json")
            .exists());
    }

    #[test]
    fn local_integration_staging_does_not_duplicate_upstream_integrations() {
        let root = tempdir().unwrap();
        let config = test_config(root.path());
        let wrapper_src = root.path().join("wrapper-src");
        let integration_config = root.path().join("port-integrations.json");
        write_local_integration(root.path(), "model-provider-switcher");
        std::fs::create_dir_all(wrapper_src.join("port-integrations/model-provider-switcher"))
            .unwrap();
        std::fs::write(
            wrapper_src.join("port-integrations/model-provider-switcher/integration.json"),
            r#"{"id":"model-provider-switcher"}"#,
        )
        .unwrap();
        std::fs::write(
            &integration_config,
            r#"{"enabled":["model-provider-switcher"]}"#,
        )
        .unwrap();

        stage_enabled_local_integrations(&config, &wrapper_src, Some(&integration_config)).unwrap();

        assert!(!wrapper_src
            .join("port-integrations/local/model-provider-switcher/integration.json")
            .exists());
    }

    #[test]
    fn malformed_integration_config_does_not_block_local_integration_staging() {
        let root = tempdir().unwrap();
        let config = test_config(root.path());
        let wrapper_src = root.path().join("wrapper-src");
        let integration_config = root.path().join("port-integrations.json");
        write_local_integration(root.path(), "model-provider-switcher");
        std::fs::create_dir_all(wrapper_src.join("port-integrations")).unwrap();
        std::fs::write(&integration_config, "{not json").unwrap();

        stage_enabled_local_integrations(&config, &wrapper_src, Some(&integration_config)).unwrap();

        assert!(!wrapper_src
            .join("port-integrations/local/model-provider-switcher/integration.json")
            .exists());
    }

    #[tokio::test]
    async fn dev_mode_candidate_is_a_noop_to_avoid_downgrade() {
        let root = tempdir().unwrap();
        let config = test_config(root.path());
        let paths = test_paths(root.path());
        let mut state = PersistedState::new(true);
        state.wrapper_dev_mode = Some(true);
        state.candidate_wrapper_commit = Some("a".repeat(40));
        state.candidate_wrapper_version = Some("0.9.0".to_string());

        run_apply_wrapper_update(&config, &mut state, &paths)
            .await
            .expect("dev-mode apply should silently skip");

        assert_eq!(state.status, UpdateStatus::Idle);
        assert_eq!(state.wrapper_dev_mode, Some(true));
        let expected_commit = "a".repeat(40);
        assert_eq!(
            state.candidate_wrapper_commit.as_deref(),
            Some(expected_commit.as_str())
        );
        assert_eq!(state.candidate_wrapper_version.as_deref(), Some("0.9.0"));
    }

    #[test]
    fn packaged_wrapper_source_inherits_generated_builder_payload() -> Result<()> {
        let root = tempdir()?;
        let config = test_config(root.path());
        let source_node = config.builder_bundle_root.join("node-runtime/bin/node");
        std::fs::create_dir_all(source_node.parent().unwrap())?;
        std::fs::write(&source_node, b"managed node")?;

        let wrapper_src = root.path().join("wrapper-src");
        std::fs::create_dir_all(&wrapper_src)?;

        seed_packaged_builder_payload(&config, &wrapper_src)?;

        assert_eq!(
            std::fs::read(wrapper_src.join("node-runtime/bin/node"))?,
            b"managed node"
        );
        Ok(())
    }

    #[test]
    fn wrapper_source_fetches_pinned_candidate_commit_for_shallow_clone() -> Result<()> {
        let _g = env_lock();
        let root = tempdir()?;
        let paths = test_paths(root.path());
        let mut config = test_config(root.path());
        config.wrapper_remote = "https://example.com/codex-app-linux.git".to_string();
        let bin_dir = root.path().join("bin");
        std::fs::create_dir_all(&bin_dir)?;
        let fake_git = bin_dir.join("git");
        let log_path = root.path().join("git-args.log");
        std::fs::write(
            &fake_git,
            r#"#!/bin/sh
printf '%s\n' "$*" >> "$CODEX_TEST_GIT_ARGS_LOG"
if [ "$1" = "clone" ]; then
  dest=""
  for arg in "$@"; do dest="$arg"; done
  mkdir -p "$dest/.git"
fi
exit 0
"#,
        )?;
        let mut permissions = std::fs::metadata(&fake_git)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&fake_git, permissions)?;

        let old_path = std::env::var_os("PATH");
        let old_log = std::env::var_os("CODEX_TEST_GIT_ARGS_LOG");
        let mut path_entries = vec![bin_dir];
        if let Some(path) = old_path.as_ref() {
            path_entries.extend(std::env::split_paths(path));
        }
        std::env::set_var("PATH", std::env::join_paths(path_entries)?);
        std::env::set_var("CODEX_TEST_GIT_ARGS_LOG", &log_path);

        let result = ensure_wrapper_source(&config, &paths, Some("abc123def456"));

        if let Some(path) = old_path {
            std::env::set_var("PATH", path);
        } else {
            std::env::remove_var("PATH");
        }
        if let Some(value) = old_log {
            std::env::set_var("CODEX_TEST_GIT_ARGS_LOG", value);
        } else {
            std::env::remove_var("CODEX_TEST_GIT_ARGS_LOG");
        }

        result?;
        let log = std::fs::read_to_string(log_path)?;
        assert!(log.contains(
            "fetch --depth 1 --quiet https://example.com/codex-app-linux.git abc123def456"
        ));
        assert!(log.contains("reset --hard --quiet abc123def456"));
        Ok(())
    }

    #[test]
    fn wrapper_rebuild_dmg_trust_populates_package_builder_digest() -> Result<()> {
        let root = tempdir()?;
        let config = test_config(root.path());
        let paths = test_paths(root.path());
        let mut state = PersistedState::new(true);
        let dmg_path = root.path().join("Codex.dmg");
        std::fs::write(&dmg_path, b"trusted wrapper rebuild dmg")?;
        let dmg_sha256 = package_verification::file_sha256(&dmg_path)?;
        let manifest_path = trust::trusted_dmg_manifest_path(&config.builder_bundle_root);
        std::fs::create_dir_all(manifest_path.parent().unwrap())?;
        std::fs::write(
            &manifest_path,
            format!(
                "{{\"schema_version\":1,\"dmgs\":[{{\"url\":\"{}\",\"version\":\"26.527.31326\",\"sha256\":\"{}\"}}]}}\n",
                config.dmg_url, dmg_sha256
            ),
        )?;

        trust_dmg_for_wrapper_rebuild(&config, &mut state, &paths, &dmg_path, None)?;

        assert_eq!(state.dmg_sha256.as_deref(), Some(dmg_sha256.as_str()));
        let verification = state.dmg_verification.as_ref().unwrap();
        assert_eq!(verification.result, DmgVerificationResult::Verified);
        assert_eq!(verification.sha256.as_deref(), Some(dmg_sha256.as_str()));
        assert_eq!(
            verification.manifest_path.as_deref(),
            Some(manifest_path.as_path())
        );
        let persisted = PersistedState::load_or_default(&paths.state_file, true)?;
        assert_eq!(persisted.dmg_sha256.as_deref(), Some(dmg_sha256.as_str()));
        Ok(())
    }

    #[test]
    fn wrapper_install_uses_recorded_package_verification() -> Result<()> {
        let root = tempdir()?;
        let config = test_config(root.path());
        let mut state = PersistedState::new(true);
        let candidate_version = "2026.05.31.225946+abcdef12";
        let dmg_sha256 = "a".repeat(64);
        let workspace_dir = config.workspace_root.join("workspaces/test");
        let package_path = workspace_dir.join("dist/codex-app_26.527.31326_amd64.deb");
        std::fs::create_dir_all(package_path.parent().unwrap())?;
        std::fs::write(&package_path, b"wrapper package")?;
        let package_sha256 = package_verification::file_sha256(&package_path)?;
        state.dmg_sha256 = Some(dmg_sha256.clone());
        state.package_verification = Some(PackageVerification {
            package_kind: "deb".to_string(),
            package_path: std::fs::canonicalize(&package_path)?,
            workspace_dir: std::fs::canonicalize(&workspace_dir)?,
            package_name: "codex-app".to_string(),
            package_version: "26.527.31326".to_string(),
            sha256: package_sha256.clone(),
            candidate_version: candidate_version.to_string(),
            dmg_sha256,
            verified_at: chrono::Utc::now(),
        });

        let expected = expected_package_for_wrapper_install(
            &config,
            &state,
            &package_path,
            candidate_version,
        )?;

        assert_eq!(expected.sha256(), package_sha256);
        assert_eq!(expected.package_name(), "codex-app");
        assert_eq!(expected.package_version(), "26.527.31326");
        Ok(())
    }

    #[test]
    fn wrapper_source_git_uses_non_interactive_environment() -> Result<()> {
        let _g = env_lock();
        let root = tempdir()?;
        let bin_dir = root.path().join("bin");
        std::fs::create_dir_all(&bin_dir)?;
        let fake_git = bin_dir.join("git");
        let record = root.path().join("git-env.txt");
        std::fs::write(
            &fake_git,
            r#"#!/bin/sh
{
  printf 'GIT_TERMINAL_PROMPT=%s\n' "$GIT_TERMINAL_PROMPT"
  printf 'GIT_ASKPASS=%s\n' "$GIT_ASKPASS"
  printf 'SSH_ASKPASS=%s\n' "$SSH_ASKPASS"
  printf 'GCM_INTERACTIVE=%s\n' "$GCM_INTERACTIVE"
  printf 'GIT_SSH_COMMAND=%s\n' "$GIT_SSH_COMMAND"
} > "$CODEX_TEST_GIT_ENV_RECORD"
exit 0
"#,
        )?;
        let mut permissions = std::fs::metadata(&fake_git)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&fake_git, permissions)?;

        let old_path = std::env::var_os("PATH");
        let old_record = std::env::var_os("CODEX_TEST_GIT_ENV_RECORD");
        let mut path_entries = vec![bin_dir];
        if let Some(path) = old_path.as_ref() {
            path_entries.extend(std::env::split_paths(path));
        }
        std::env::set_var("PATH", std::env::join_paths(path_entries)?);
        std::env::set_var("CODEX_TEST_GIT_ENV_RECORD", &record);

        let result = run_git(&["status"]);

        if let Some(path) = old_path {
            std::env::set_var("PATH", path);
        } else {
            std::env::remove_var("PATH");
        }
        if let Some(value) = old_record {
            std::env::set_var("CODEX_TEST_GIT_ENV_RECORD", value);
        } else {
            std::env::remove_var("CODEX_TEST_GIT_ENV_RECORD");
        }

        result?;
        let env_record = std::fs::read_to_string(record)?;
        assert!(env_record.contains("GIT_TERMINAL_PROMPT=0"));
        assert!(env_record.contains("GIT_ASKPASS=true"));
        assert!(env_record.contains("SSH_ASKPASS=true"));
        assert!(env_record.contains("GCM_INTERACTIVE=never"));
        assert!(env_record.contains("-oBatchMode=yes"));
        Ok(())
    }

    #[test]
    fn wrapper_source_git_times_out() -> Result<()> {
        let _g = env_lock();
        let root = tempdir()?;
        let bin_dir = root.path().join("bin");
        std::fs::create_dir_all(&bin_dir)?;
        let fake_git = bin_dir.join("git");
        std::fs::write(&fake_git, "#!/bin/sh\nsleep 60\n")?;
        let mut permissions = std::fs::metadata(&fake_git)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&fake_git, permissions)?;

        let old_path = std::env::var_os("PATH");
        let mut path_entries = vec![bin_dir];
        if let Some(path) = old_path.as_ref() {
            path_entries.extend(std::env::split_paths(path));
        }
        std::env::set_var("PATH", std::env::join_paths(path_entries)?);
        let started = Instant::now();

        let error = run_git(&["clone", "git@example.invalid:repo.git"])
            .expect_err("prompting git command should time out");

        if let Some(path) = old_path {
            std::env::set_var("PATH", path);
        } else {
            std::env::remove_var("PATH");
        }

        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(error.to_string().contains("timed out"));
        Ok(())
    }
}
