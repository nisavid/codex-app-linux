//! CLI discovery and prelaunch update checks for the user-installed Codex CLI.

use crate::{
    config::{RuntimeConfig, RuntimePaths},
    state::{CliPathSource, CliStatus, PersistedState},
};
use anyhow::{anyhow, Context, Result};
use chrono::{Duration, Utc};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    ffi::OsStr,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tracing::{info, warn};

const CLI_PACKAGE_NAME: &str = "@openai/codex";
const CLI_VERSION_CHECK_TTL: Duration = Duration::hours(1);

#[derive(Debug)]
pub struct InvalidConfiguredCliPath {
    source: CliPathSource,
    path: PathBuf,
    requires_absolute: bool,
}

impl InvalidConfiguredCliPath {
    fn new(source: CliPathSource, path: &Path) -> Self {
        Self {
            source,
            path: path.to_path_buf(),
            requires_absolute: false,
        }
    }

    fn relative(source: CliPathSource, path: &Path) -> Self {
        Self {
            source,
            path: path.to_path_buf(),
            requires_absolute: true,
        }
    }
}

impl std::fmt::Display for InvalidConfiguredCliPath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.requires_absolute {
            return write!(
                formatter,
                "Invalid {} Codex CLI path {}: configured path must be absolute",
                cli_path_source_label(&self.source),
                self.path.display()
            );
        }
        write!(
            formatter,
            "Invalid {} Codex CLI path {}: expected an executable regular file",
            cli_path_source_label(&self.source),
            self.path.display()
        )
    }
}

impl std::error::Error for InvalidConfiguredCliPath {}

pub fn is_invalid_configured_cli_path_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<InvalidConfiguredCliPath>().is_some()
}

pub(crate) fn is_usable_cli_path(path: &Path) -> bool {
    is_executable(path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreflightOutcome {
    pub cli_path: PathBuf,
    pub cli_path_source: CliPathSource,
    pub installed_version: String,
    pub latest_version: Option<String>,
    pub updated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliPathResolution {
    path: PathBuf,
    source: CliPathSource,
}

#[derive(Debug, Clone)]
struct ResolveCliPathRequest<'a> {
    explicit_path: Option<&'a Path>,
    env_path: Option<&'a Path>,
    config_path: Option<&'a Path>,
    persisted_path: Option<&'a Path>,
    path_env: Option<&'a OsStr>,
    home: Option<&'a Path>,
    fallback_env: Vec<(String, PathBuf)>,
}

pub fn preflight(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
    explicit_cli_path: Option<PathBuf>,
    allow_install_missing: bool,
) -> Result<PreflightOutcome> {
    let requested_path = explicit_cli_path.as_deref();
    let resolution = match resolve_runtime_cli_path(config, state, requested_path)? {
        Some(resolution) => resolution,
        None if allow_install_missing => install_missing_cli(config, state, paths, requested_path)?,
        None => anyhow::bail!(
            "Codex CLI not found in explicit, environment, config, persisted, PATH, or known fallback locations"
        ),
    };
    let cli_path = resolution.path.clone();
    let cached_installed_version = state.cli_installed_version.clone();
    state.cli_path = Some(cli_path.clone());
    state.cli_path_source = resolution.source.clone();
    state.cli_installed_version = None;
    persist_state(paths, state)?;
    let installed_version = match read_installed_version(&cli_path) {
        Ok(version) => version,
        Err(error) => {
            state.cli_status = CliStatus::Failed;
            state.cli_error_message = Some(format!(
                "Could not read the installed {} version: {error}",
                CLI_PACKAGE_NAME
            ));
            persist_state(paths, state)?;
            return Err(error);
        }
    };
    state.cli_installed_version = Some(installed_version.clone());
    persist_state(paths, state)?;

    if should_skip_latest_version_check(
        state,
        cached_installed_version.as_deref(),
        &installed_version,
    ) {
        info!(
            installed_version,
            "skipping Codex CLI registry lookup because the cached result is still fresh"
        );
        refresh_cli_status_from_latest(state, &installed_version);
        state.cli_error_message = None;
        persist_state(paths, state)?;
        return Ok(PreflightOutcome {
            cli_path,
            cli_path_source: resolution.source,
            installed_version,
            latest_version: state.cli_latest_version.clone(),
            updated: false,
        });
    }

    state.cli_last_check_at = Some(Utc::now());
    state.cli_error_message = None;
    state.cli_status = CliStatus::Checking;
    persist_state(paths, state)?;

    let latest_version = match read_latest_version() {
        Ok(version) => version,
        Err(error) => {
            state.cli_status = CliStatus::Unknown;
            state.cli_latest_version = None;
            state.cli_error_message = Some(format!(
                "Could not check the latest {} version: {error}",
                CLI_PACKAGE_NAME
            ));
            persist_state(paths, state)?;
            warn!(?error, "unable to check latest Codex CLI version");
            return Ok(PreflightOutcome {
                cli_path,
                cli_path_source: resolution.source,
                installed_version,
                latest_version: None,
                updated: false,
            });
        }
    };

    state.cli_latest_version = Some(latest_version.clone());
    if installed_version == latest_version {
        state.cli_status = CliStatus::UpToDate;
        state.cli_error_message = None;
        persist_state(paths, state)?;
        return Ok(PreflightOutcome {
            cli_path,
            cli_path_source: resolution.source,
            installed_version,
            latest_version: Some(latest_version),
            updated: false,
        });
    }

    state.cli_status = CliStatus::UpdateRequired;
    persist_state(paths, state)?;
    info!(
        installed_version,
        latest_version, "Codex CLI is outdated; attempting prelaunch upgrade"
    );

    state.cli_status = CliStatus::Updating;
    persist_state(paths, state)?;
    if let Err(error) = install_latest_cli(&latest_version) {
        let message = format!("Codex CLI upgrade failed: {error}");
        persist_cli_failure(paths, state, message.clone(), false)?;
        anyhow::bail!(message);
    }

    let refreshed = match resolve_runtime_cli_path_after_install(config, state, requested_path)? {
        Some(resolution) => resolution,
        None => match resolve_runtime_cli_path_after_install(config, state, None)? {
            Some(resolution) => resolution,
            None => {
                let message =
                    "Codex CLI disappeared after the automatic upgrade attempt".to_string();
                persist_cli_failure(paths, state, message.clone(), true)?;
                anyhow::bail!(message);
            }
        },
    };
    let refreshed_path = refreshed.path.clone();
    let refreshed_version = match read_installed_version(&refreshed_path) {
        Ok(version) => version,
        Err(error) => {
            let message = format!(
                "Could not read the installed {} version after upgrade: {error}",
                CLI_PACKAGE_NAME
            );
            persist_cli_failure(paths, state, message.clone(), true)?;
            anyhow::bail!(message);
        }
    };
    state.cli_path = Some(refreshed_path.clone());
    state.cli_path_source = refreshed.source.clone();
    state.cli_installed_version = Some(refreshed_version.clone());

    if refreshed_version != latest_version {
        let message = format!(
            "Codex CLI upgrade finished but the installed version is still {} instead of {}",
            refreshed_version, latest_version
        );
        persist_cli_failure(paths, state, message.clone(), false)?;
        anyhow::bail!(message);
    }

    state.cli_status = CliStatus::UpToDate;
    state.cli_error_message = None;
    persist_state(paths, state)?;
    Ok(PreflightOutcome {
        cli_path: refreshed_path,
        cli_path_source: refreshed.source,
        installed_version: refreshed_version,
        latest_version: Some(latest_version),
        updated: true,
    })
}

pub fn refresh_status(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
) -> Result<()> {
    let resolution = match resolve_runtime_cli_path(config, state, None)? {
        Some(resolution) => resolution,
        None => {
            state.cli_path = None;
            state.cli_path_source = CliPathSource::Unknown;
            state.cli_installed_version = None;
            state.cli_status = CliStatus::Unknown;
            state.cli_error_message = Some(
                "Codex CLI not found in configured paths, persisted path, PATH, or known fallback locations"
                    .to_string(),
            );
            persist_state(paths, state)?;
            return Ok(());
        }
    };
    let cli_path = resolution.path.clone();

    let cached_installed_version = state.cli_installed_version.clone();
    let installed_version = match read_installed_version(&cli_path) {
        Ok(version) => version,
        Err(error) => {
            state.cli_path = Some(cli_path);
            state.cli_path_source = resolution.source;
            state.cli_installed_version = None;
            state.cli_status = CliStatus::Failed;
            state.cli_error_message = Some(format!(
                "Could not read the installed {} version: {error}",
                CLI_PACKAGE_NAME
            ));
            persist_state(paths, state)?;
            warn!(?error, "unable to read installed Codex CLI version");
            return Ok(());
        }
    };

    state.cli_path = Some(cli_path);
    state.cli_path_source = resolution.source;
    state.cli_installed_version = Some(installed_version.clone());

    if should_skip_latest_version_check(
        state,
        cached_installed_version.as_deref(),
        &installed_version,
    ) {
        info!(
            installed_version,
            "skipping Codex CLI registry lookup because the cached result is still fresh"
        );
        refresh_cli_status_from_latest(state, &installed_version);
        state.cli_error_message = None;
        persist_state(paths, state)?;
        return Ok(());
    }

    state.cli_last_check_at = Some(Utc::now());
    state.cli_error_message = None;
    state.cli_status = CliStatus::Checking;
    persist_state(paths, state)?;

    match read_latest_version() {
        Ok(latest_version) => {
            state.cli_latest_version = Some(latest_version);
            refresh_cli_status_from_latest(state, &installed_version);
            state.cli_error_message = None;
        }
        Err(error) => {
            let cached_latest_matches_install = cached_latest_version_matches_install(
                state,
                cached_installed_version.as_deref(),
                &installed_version,
            );
            if cached_latest_matches_install {
                refresh_cli_status_from_latest(state, &installed_version);
            } else {
                state.cli_status = CliStatus::Unknown;
            }
            state.cli_error_message = Some(format!(
                "Could not check the latest {} version: {error}",
                CLI_PACKAGE_NAME
            ));
            warn!(?error, "unable to check latest Codex CLI version");
        }
    }

    persist_state(paths, state)
}

fn persist_state(paths: &RuntimePaths, state: &PersistedState) -> Result<()> {
    state.save(&paths.state_file)
}

fn persist_cli_failure(
    paths: &RuntimePaths,
    state: &mut PersistedState,
    message: String,
    clear_cli_metadata: bool,
) -> Result<()> {
    if clear_cli_metadata {
        state.cli_path = None;
        state.cli_path_source = CliPathSource::Unknown;
        state.cli_installed_version = None;
    }
    state.cli_status = CliStatus::Failed;
    state.cli_error_message = Some(message);
    persist_state(paths, state)
}

fn resolve_cli_path(request: &ResolveCliPathRequest<'_>) -> Result<Option<CliPathResolution>> {
    for (source, path) in [
        (CliPathSource::Explicit, request.explicit_path),
        (CliPathSource::Env, request.env_path),
        (CliPathSource::Config, request.config_path),
    ] {
        if let Some(path) = path {
            if matches!(&source, CliPathSource::Env | CliPathSource::Config) && !path.is_absolute()
            {
                return Err(InvalidConfiguredCliPath::relative(source, path).into());
            }
            if is_executable(path) {
                return Ok(Some(CliPathResolution {
                    path: normalize_cli_path(path),
                    source,
                }));
            }
            return Err(InvalidConfiguredCliPath::new(source, path).into());
        }
    }

    if let Some(path) = request.persisted_path {
        if path.is_absolute() && is_executable(path) {
            return Ok(Some(CliPathResolution {
                path: normalize_cli_path(path),
                source: CliPathSource::Persisted,
            }));
        }
    }

    if let Some(path_env) = request.path_env {
        if let Some(path) = find_in_path("codex", path_env) {
            return Ok(Some(CliPathResolution {
                path: normalize_cli_path(&path),
                source: CliPathSource::Path,
            }));
        }
    }

    for candidate in known_cli_paths(request) {
        if is_executable(&candidate) {
            return Ok(Some(CliPathResolution {
                path: normalize_cli_path(&candidate),
                source: CliPathSource::KnownPath,
            }));
        }
    }

    Ok(None)
}

fn resolve_cli_path_after_install(
    request: &ResolveCliPathRequest<'_>,
) -> Result<Option<CliPathResolution>> {
    let mut request = request.clone();
    request.persisted_path = None;
    resolve_cli_path(&request)
}

fn resolve_runtime_cli_path(
    config: &RuntimeConfig,
    state: &PersistedState,
    explicit_path: Option<&Path>,
) -> Result<Option<CliPathResolution>> {
    let env_path = std::env::var_os("CODEX_CLI_PATH")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    let path_env = command_path_env();
    let home = home_dir();
    let request = ResolveCliPathRequest {
        explicit_path,
        env_path: env_path.as_deref(),
        config_path: config.cli_path.as_deref(),
        persisted_path: state.cli_path.as_deref(),
        path_env: Some(path_env.as_os_str()),
        home: home.as_deref(),
        fallback_env: current_fallback_env(),
    };
    resolve_cli_path(&request)
}

fn resolve_runtime_cli_path_after_install(
    config: &RuntimeConfig,
    state: &PersistedState,
    explicit_path: Option<&Path>,
) -> Result<Option<CliPathResolution>> {
    let env_path = std::env::var_os("CODEX_CLI_PATH")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    let path_env = command_path_env();
    let home = home_dir();
    let request = ResolveCliPathRequest {
        explicit_path,
        env_path: env_path.as_deref(),
        config_path: config.cli_path.as_deref(),
        persisted_path: state.cli_path.as_deref(),
        path_env: Some(path_env.as_os_str()),
        home: home.as_deref(),
        fallback_env: current_fallback_env(),
    };
    resolve_cli_path_after_install(&request)
}

fn current_fallback_env() -> Vec<(String, PathBuf)> {
    [
        "PNPM_HOME",
        "BUN_INSTALL",
        "VOLTA_HOME",
        "ASDF_DATA_DIR",
        "MISE_DATA_DIR",
    ]
    .into_iter()
    .filter_map(|name| {
        std::env::var_os(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .map(|path| (name.to_string(), path))
    })
    .collect()
}

fn known_cli_paths(request: &ResolveCliPathRequest<'_>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    for (name, value) in &request.fallback_env {
        if !value.is_absolute() {
            continue;
        }
        match name.as_str() {
            "PNPM_HOME" => candidates.push(value.join("codex")),
            "BUN_INSTALL" => candidates.push(value.join("bin/codex")),
            "VOLTA_HOME" => candidates.push(value.join("bin/codex")),
            "ASDF_DATA_DIR" => candidates.push(value.join("shims/codex")),
            "MISE_DATA_DIR" => candidates.push(value.join("shims/codex")),
            _ => {}
        }
    }

    if let Some(home) = request.home {
        candidates.extend([
            home.join(".local/bin/codex"),
            home.join(".local/share/pnpm/codex"),
            home.join(".bun/bin/codex"),
            home.join(".volta/bin/codex"),
            home.join(".asdf/shims/codex"),
            home.join(".local/share/mise/shims/codex"),
            home.join(".nix-profile/bin/codex"),
            home.join(".local/state/nix/profile/bin/codex"),
            home.join(".yarn/bin/codex"),
        ]);
    }

    candidates.push(PathBuf::from("/home/linuxbrew/.linuxbrew/bin/codex"));
    candidates
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

fn normalize_cli_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

fn cli_path_source_label(source: &CliPathSource) -> &'static str {
    match source {
        CliPathSource::Explicit => "explicit",
        CliPathSource::Env => "environment",
        CliPathSource::Config => "config",
        CliPathSource::Persisted => "persisted",
        CliPathSource::Path => "PATH",
        CliPathSource::KnownPath => "known fallback",
        CliPathSource::AutoInstall => "automatic install",
        CliPathSource::Unknown => "unknown",
    }
}

fn should_skip_latest_version_check(
    state: &PersistedState,
    cached_installed_version: Option<&str>,
    installed_version: &str,
) -> bool {
    let Some(last_check_at) = state.cli_last_check_at else {
        return false;
    };
    if !cached_latest_version_matches_install(state, cached_installed_version, installed_version) {
        return false;
    }

    Utc::now().signed_duration_since(last_check_at) < CLI_VERSION_CHECK_TTL
}

fn cached_latest_version_matches_install(
    state: &PersistedState,
    cached_installed_version: Option<&str>,
    installed_version: &str,
) -> bool {
    state.cli_latest_version.is_some() && cached_installed_version == Some(installed_version)
}

fn refresh_cli_status_from_latest(state: &mut PersistedState, installed_version: &str) {
    state.cli_status = match state.cli_latest_version.as_deref() {
        Some(latest_version) if latest_version == installed_version => CliStatus::UpToDate,
        Some(_) => CliStatus::UpdateRequired,
        None => CliStatus::Unknown,
    };
}

fn read_installed_version(cli_path: &Path) -> Result<String> {
    let primary = run_command(cli_path, ["--version"])?;
    if let Some(version) = extract_version(&primary) {
        return Ok(version);
    }

    let fallback = run_command(cli_path, ["version"])?;
    extract_version(&fallback).ok_or_else(|| {
        anyhow!(
            "Codex CLI returned an unparseable version string: {}",
            fallback.trim()
        )
    })
}

fn read_latest_version() -> Result<String> {
    let npm = npm_program();
    let output = Command::new(&npm)
        .env("PATH", command_path_env())
        .args(["view", CLI_PACKAGE_NAME, "version"])
        .output()
        .with_context(|| format!("Failed to spawn {}", npm.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        anyhow::bail!(
            "{} view {} version failed with {}{}",
            npm.display(),
            CLI_PACKAGE_NAME,
            output.status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        );
    }

    extract_version(&String::from_utf8_lossy(&output.stdout)).ok_or_else(|| {
        anyhow!(
            "{} view {} version returned an unparseable version string",
            npm.display(),
            CLI_PACKAGE_NAME
        )
    })
}

fn install_latest_cli(latest_version: &str) -> Result<()> {
    let npm = npm_program();
    let package_spec = format!("{CLI_PACKAGE_NAME}@{latest_version}");
    let global_args = vec![
        OsString::from("install"),
        OsString::from("-g"),
        OsString::from(&package_spec),
    ];

    match run_npm_command(&npm, &global_args) {
        Ok(()) => Ok(()),
        Err(global_error) => {
            warn!(
                ?global_error,
                "global npm install failed; retrying Codex CLI upgrade with a user-local prefix"
            );

            let local_prefix = local_npm_prefix();
            fs::create_dir_all(&local_prefix).with_context(|| {
                format!(
                    "Failed to create local npm prefix {}",
                    local_prefix.display()
                )
            })?;

            let local_args = vec![
                OsString::from("install"),
                OsString::from("-g"),
                OsString::from("--prefix"),
                local_prefix.as_os_str().to_os_string(),
                OsString::from(&package_spec),
            ];

            run_npm_command(&npm, &local_args).with_context(|| {
                format!(
                    "npm install -g failed first ({global_error}); fallback install into {} also failed",
                    local_prefix.display()
                )
            })
        }
    }
}

fn install_missing_cli(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
    requested_path: Option<&Path>,
) -> Result<CliPathResolution> {
    state.cli_status = CliStatus::Updating;
    persist_state(paths, state)?;

    let latest_version = match read_latest_version() {
        Ok(version) => version,
        Err(error) => {
            let message = format!(
                "Could not check the latest {} version before automatic installation: {error}",
                CLI_PACKAGE_NAME
            );
            persist_cli_failure(paths, state, message.clone(), true)?;
            anyhow::bail!(message);
        }
    };
    state.cli_latest_version = Some(latest_version.clone());
    persist_state(paths, state)?;

    info!(
        latest_version,
        "Codex CLI is missing; attempting automatic installation"
    );
    if let Err(error) = install_latest_cli(&latest_version) {
        let message = format!("Codex CLI automatic installation failed: {error}");
        persist_cli_failure(paths, state, message.clone(), true)?;
        anyhow::bail!(message);
    }

    let mut resolution =
        match resolve_runtime_cli_path_after_install(config, state, requested_path)? {
            Some(resolution) => resolution,
            None => match resolve_runtime_cli_path_after_install(config, state, None)? {
                Some(resolution) => resolution,
                None => {
                    let message =
                        "Codex CLI installed but could not be found afterwards".to_string();
                    persist_cli_failure(paths, state, message.clone(), true)?;
                    anyhow::bail!(message);
                }
            },
        };
    resolution.source = CliPathSource::AutoInstall;

    Ok(resolution)
}

fn run_command<I, S>(program: &Path, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = Command::new(program)
        .env("PATH", command_path_env())
        .args(args)
        .output()
        .with_context(|| format!("Failed to spawn {}", program.display()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        anyhow::bail!(
            "{} exited with {}{}",
            program.display(),
            output.status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn extract_version(raw: &str) -> Option<String> {
    raw.split_whitespace()
        .find_map(normalize_version_token)
        .or_else(|| {
            let trimmed = raw.trim();
            normalize_version_token(trimmed)
        })
}

fn normalize_version_token(token: &str) -> Option<String> {
    let trimmed = token.trim_matches(|ch: char| {
        !ch.is_ascii_alphanumeric() && ch != '.' && ch != '-' && ch != '_'
    });
    let trimmed = trimmed.strip_prefix('v').unwrap_or(trimmed);
    if trimmed.is_empty() || !trimmed.contains('.') {
        return None;
    }
    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_')
    {
        return None;
    }
    if !trimmed.chars().any(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(trimmed.to_string())
}

fn npm_program() -> PathBuf {
    find_in_path("npm", &command_path_env()).unwrap_or_else(|| PathBuf::from("npm"))
}

fn local_npm_prefix() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".local")
}

fn run_npm_command(npm: &Path, args: &[OsString]) -> Result<()> {
    let output = Command::new(npm)
        .env("PATH", command_path_env())
        .args(args)
        .output()
        .with_context(|| format!("Failed to spawn {}", npm.display()))?;

    anyhow::ensure!(
        output.status.success(),
        "{} {} failed with {}{}",
        npm.display(),
        format_command_args(args),
        output.status,
        format_command_output(&output)
    );

    Ok(())
}

fn format_command_args(args: &[OsString]) -> String {
    args.iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_command_output(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return format!(": {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        String::new()
    } else {
        format!(": {stdout}")
    }
}

fn find_in_path(name: &str, path_env: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path_env).find_map(|entry| {
        if entry.as_os_str().is_empty() || !entry.is_absolute() {
            return None;
        }
        let candidate = entry.join(name);
        if is_executable(&candidate) {
            Some(candidate)
        } else {
            None
        }
    })
}

fn command_path_env() -> OsString {
    std::env::var_os("PATH").unwrap_or_default()
}

/// Checks for a regular file with at least one execute bit.
///
/// This deliberately avoids an extra permission-check dependency. It does not
/// prove the current user can execute the file; actual execution will still
/// fail at runtime if ownership or ACLs deny access. A stricter check would use
/// `nix::unistd::access(path, AccessFlags::X_OK)`.
fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    is_executable_metadata(&metadata)
}

#[cfg(unix)]
fn is_executable_metadata(metadata: &fs::Metadata) -> bool {
    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable_metadata(metadata: &fs::Metadata) -> bool {
    metadata.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{RuntimeConfig, RuntimePaths},
        state::{CliPathSource, CliStatus, PersistedState},
    };
    use chrono::Utc;
    use std::{fs, os::unix::fs::PermissionsExt, path::Path};
    use tempfile::tempdir;

    fn write_executable_script(path: &Path, contents: &str) -> Result<()> {
        fs::write(path, contents)?;
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)?;
        Ok(())
    }

    fn test_runtime_paths(root: &Path) -> RuntimePaths {
        RuntimePaths {
            config_file: root.join("config/config.toml"),
            state_file: root.join("state/state.json"),
            log_file: root.join("state/service.log"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            config_dir: root.join("config"),
        }
    }

    fn test_runtime_config(paths: &RuntimePaths) -> RuntimeConfig {
        RuntimeConfig::default_with_paths(paths)
    }

    #[test]
    fn extracts_plain_semver() {
        assert_eq!(extract_version("0.34.1"), Some("0.34.1".to_string()));
    }

    #[test]
    fn extracts_prefixed_semver() {
        assert_eq!(
            extract_version("codex-cli v0.34.1"),
            Some("0.34.1".to_string())
        );
    }

    #[test]
    fn ignores_non_version_text() {
        assert_eq!(extract_version("Codex CLI"), None);
    }

    #[test]
    fn skips_registry_lookup_when_previous_check_is_fresh_for_same_cli_version() {
        let mut state = PersistedState::new(true);
        state.cli_installed_version = Some("0.42.0".to_string());
        state.cli_latest_version = Some("0.42.1".to_string());
        state.cli_last_check_at = Some(Utc::now() - Duration::minutes(30));

        assert!(should_skip_latest_version_check(
            &state,
            Some("0.42.0"),
            "0.42.0"
        ));
    }

    #[test]
    fn does_not_skip_registry_lookup_when_cli_version_changed() {
        let mut state = PersistedState::new(true);
        state.cli_installed_version = Some("0.42.0".to_string());
        state.cli_latest_version = Some("0.42.1".to_string());
        state.cli_last_check_at = Some(Utc::now() - Duration::minutes(30));

        assert!(!should_skip_latest_version_check(
            &state,
            Some("0.42.0"),
            "0.43.0"
        ));
    }

    #[test]
    fn does_not_skip_registry_lookup_when_cached_check_is_stale() {
        let mut state = PersistedState::new(true);
        state.cli_installed_version = Some("0.42.0".to_string());
        state.cli_latest_version = Some("0.42.0".to_string());
        state.cli_last_check_at = Some(Utc::now() - Duration::hours(2));

        assert!(!should_skip_latest_version_check(
            &state,
            Some("0.42.0"),
            "0.42.0"
        ));
    }

    #[test]
    fn does_not_skip_registry_lookup_without_cached_latest_version() {
        let mut state = PersistedState::new(true);
        state.cli_installed_version = Some("0.42.0".to_string());
        state.cli_last_check_at = Some(Utc::now() - Duration::minutes(30));

        assert!(!should_skip_latest_version_check(
            &state,
            Some("0.42.0"),
            "0.42.0"
        ));
    }

    #[test]
    fn resolves_persisted_cli_path_without_environment() -> Result<()> {
        let temp = tempdir()?;
        let codex_path = temp.path().join("codex");
        write_executable_script(
            &codex_path,
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'codex-cli v0.42.0'\n  exit 0\nfi\nexit 1\n",
        )?;

        let resolution = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: None,
            env_path: None,
            config_path: None,
            persisted_path: Some(codex_path.as_path()),
            path_env: None,
            home: None,
            fallback_env: Vec::new(),
        })?
        .expect("Codex CLI path");

        assert_eq!(resolution.path, codex_path);
        assert_eq!(resolution.source, CliPathSource::Persisted);
        Ok(())
    }

    #[test]
    fn preflight_uses_cached_latest_for_fresh_explicit_cli_path() -> Result<()> {
        let temp = tempdir()?;
        let paths = test_runtime_paths(temp.path());
        let config = test_runtime_config(&paths);
        paths.ensure_dirs()?;

        let codex_path = temp.path().join("codex");
        write_executable_script(
            &codex_path,
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo 'codex-cli v0.42.0'\n  exit 0\nfi\nexit 1\n",
        )?;

        let mut state = PersistedState::new(true);
        state.cli_installed_version = Some("0.42.0".to_string());
        state.cli_latest_version = Some("0.42.0".to_string());
        state.cli_last_check_at = Some(Utc::now() - Duration::minutes(5));
        state.cli_status = CliStatus::Unknown;
        state.cli_error_message = Some("previous error".to_string());

        let outcome = preflight(&config, &mut state, &paths, Some(codex_path.clone()), false)?;

        assert_eq!(outcome.cli_path, codex_path);
        assert_eq!(outcome.installed_version, "0.42.0");
        assert_eq!(outcome.latest_version.as_deref(), Some("0.42.0"));
        assert!(!outcome.updated);
        assert_eq!(state.cli_latest_version.as_deref(), Some("0.42.0"));
        assert_eq!(state.cli_status, CliStatus::UpToDate);
        assert_eq!(state.cli_error_message, None);
        Ok(())
    }

    #[test]
    fn preflight_persists_resolved_path_before_version_probe_failure() -> Result<()> {
        let temp = tempdir()?;
        let paths = test_runtime_paths(temp.path());
        let mut config = test_runtime_config(&paths);
        paths.ensure_dirs()?;

        let stale_cli = temp.path().join("stale/codex");
        let broken_cli = temp.path().join("configured/codex");
        fs::create_dir_all(stale_cli.parent().expect("parent"))?;
        fs::create_dir_all(broken_cli.parent().expect("parent"))?;
        write_executable_script(&stale_cli, "#!/bin/sh\necho 'codex-cli v0.41.0'\n")?;
        write_executable_script(
            &broken_cli,
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  exit 19\nfi\nexit 1\n",
        )?;
        config.cli_path = Some(broken_cli.clone());

        let mut state = PersistedState::new(true);
        state.cli_path = Some(stale_cli.clone());
        state.cli_path_source = CliPathSource::Persisted;
        state.cli_installed_version = Some("0.41.0".to_string());

        let result = preflight(&config, &mut state, &paths, None, false);

        result.expect_err("broken resolved CLI should fail version probe");
        assert_eq!(state.cli_path.as_deref(), Some(broken_cli.as_path()));
        assert_eq!(state.cli_path_source, CliPathSource::Config);
        assert_eq!(state.cli_installed_version, None);
        assert_eq!(state.cli_status, CliStatus::Failed);
        assert!(state
            .cli_error_message
            .as_deref()
            .unwrap_or_default()
            .contains("Could not read the installed @openai/codex version"));
        Ok(())
    }

    #[test]
    fn resolves_cli_path_by_source_precedence() -> Result<()> {
        let temp = tempdir()?;
        let explicit = temp.path().join("explicit/codex");
        let env = temp.path().join("env/codex");
        let config_path = temp.path().join("config/codex");
        let persisted = temp.path().join("persisted/codex");
        let path_dir = temp.path().join("path");
        let path_cli = path_dir.join("codex");
        for path in [&explicit, &env, &config_path, &persisted, &path_cli] {
            fs::create_dir_all(path.parent().expect("parent"))?;
            write_executable_script(path, "#!/bin/sh\necho 'codex-cli v0.42.0'\n")?;
        }

        let outcome = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: Some(explicit.as_path()),
            env_path: Some(env.as_path()),
            config_path: Some(config_path.as_path()),
            persisted_path: Some(persisted.as_path()),
            path_env: Some(path_dir.as_os_str()),
            home: Some(temp.path()),
            fallback_env: Vec::new(),
        })?
        .expect("Codex CLI path");
        assert_eq!(outcome.path, explicit);
        assert_eq!(outcome.source, CliPathSource::Explicit);

        let outcome = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: None,
            env_path: Some(env.as_path()),
            config_path: Some(config_path.as_path()),
            persisted_path: Some(persisted.as_path()),
            path_env: Some(path_dir.as_os_str()),
            home: Some(temp.path()),
            fallback_env: Vec::new(),
        })?
        .expect("Codex CLI path");
        assert_eq!(outcome.path, env);
        assert_eq!(outcome.source, CliPathSource::Env);

        let outcome = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: None,
            env_path: None,
            config_path: Some(config_path.as_path()),
            persisted_path: Some(persisted.as_path()),
            path_env: Some(path_dir.as_os_str()),
            home: Some(temp.path()),
            fallback_env: Vec::new(),
        })?
        .expect("Codex CLI path");
        assert_eq!(outcome.path, config_path);
        assert_eq!(outcome.source, CliPathSource::Config);

        let outcome = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: None,
            env_path: None,
            config_path: None,
            persisted_path: Some(persisted.as_path()),
            path_env: Some(path_dir.as_os_str()),
            home: Some(temp.path()),
            fallback_env: Vec::new(),
        })?
        .expect("Codex CLI path");
        assert_eq!(outcome.path, persisted);
        assert_eq!(outcome.source, CliPathSource::Persisted);

        let outcome = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: None,
            env_path: None,
            config_path: None,
            persisted_path: None,
            path_env: Some(path_dir.as_os_str()),
            home: Some(temp.path()),
            fallback_env: Vec::new(),
        })?
        .expect("Codex CLI path");
        assert_eq!(outcome.path, path_cli);
        assert_eq!(outcome.source, CliPathSource::Path);
        Ok(())
    }

    #[test]
    fn stale_persisted_path_falls_back_to_known_path() -> Result<()> {
        let temp = tempdir()?;
        let local_bin = temp.path().join(".local/bin");
        let known_cli = local_bin.join("codex");
        fs::create_dir_all(&local_bin)?;
        write_executable_script(&known_cli, "#!/bin/sh\necho 'codex-cli v0.42.0'\n")?;

        let stale = temp.path().join("stale/codex");
        let outcome = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: None,
            env_path: None,
            config_path: None,
            persisted_path: Some(stale.as_path()),
            path_env: None,
            home: Some(temp.path()),
            fallback_env: Vec::new(),
        })?
        .expect("Codex CLI path");

        assert_eq!(outcome.path, known_cli);
        assert_eq!(outcome.source, CliPathSource::KnownPath);
        Ok(())
    }

    #[test]
    fn post_install_resolution_ignores_stale_persisted_path() -> Result<()> {
        let temp = tempdir()?;
        let persisted = temp.path().join("persisted/codex");
        let path_dir = temp.path().join("path");
        let path_cli = path_dir.join("codex");
        fs::create_dir_all(persisted.parent().expect("parent"))?;
        fs::create_dir_all(&path_dir)?;
        write_executable_script(&persisted, "#!/bin/sh\necho 'codex-cli v0.41.0'\n")?;
        write_executable_script(&path_cli, "#!/bin/sh\necho 'codex-cli v0.42.0'\n")?;

        let request = ResolveCliPathRequest {
            explicit_path: None,
            env_path: None,
            config_path: None,
            persisted_path: Some(persisted.as_path()),
            path_env: Some(path_dir.as_os_str()),
            home: None,
            fallback_env: Vec::new(),
        };

        let normal = resolve_cli_path(&request)?.expect("normal Codex CLI path");
        assert_eq!(normal.path, persisted);
        assert_eq!(normal.source, CliPathSource::Persisted);

        let after_install =
            resolve_cli_path_after_install(&request)?.expect("post-install Codex CLI path");
        assert_eq!(after_install.path, path_cli);
        assert_eq!(after_install.source, CliPathSource::Path);
        Ok(())
    }

    #[test]
    fn relative_persisted_path_is_ignored() -> Result<()> {
        let cwd = std::env::current_dir()?;
        let temp = tempfile::Builder::new()
            .prefix(".codex-cli-relative-persisted-")
            .tempdir_in(&cwd)?;
        let persisted = temp.path().join("persisted/codex");
        let path_dir = temp.path().join("path");
        let path_cli = path_dir.join("codex");
        fs::create_dir_all(persisted.parent().expect("parent"))?;
        fs::create_dir_all(&path_dir)?;
        write_executable_script(&persisted, "#!/bin/sh\necho 'codex-cli v0.41.0'\n")?;
        write_executable_script(&path_cli, "#!/bin/sh\necho 'codex-cli v0.42.0'\n")?;
        let relative_persisted = persisted.strip_prefix(&cwd)?;

        let outcome = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: None,
            env_path: None,
            config_path: None,
            persisted_path: Some(relative_persisted),
            path_env: Some(path_dir.as_os_str()),
            home: None,
            fallback_env: Vec::new(),
        })?
        .expect("Codex CLI path");

        assert_eq!(outcome.path, path_cli);
        assert_eq!(outcome.source, CliPathSource::Path);
        Ok(())
    }

    #[test]
    fn invalid_explicit_and_config_paths_fail_loudly() {
        let temp = tempdir().expect("tempdir");
        let invalid = temp.path().join("codex");
        fs::write(&invalid, "#!/bin/sh\n").expect("write non-executable");

        let explicit_error = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: Some(invalid.as_path()),
            env_path: None,
            config_path: None,
            persisted_path: None,
            path_env: None,
            home: Some(temp.path()),
            fallback_env: Vec::new(),
        })
        .expect_err("invalid explicit path should fail");
        assert!(explicit_error.to_string().contains("explicit"));

        let config_error = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: None,
            env_path: None,
            config_path: Some(invalid.as_path()),
            persisted_path: None,
            path_env: None,
            home: Some(temp.path()),
            fallback_env: Vec::new(),
        })
        .expect_err("invalid config path should fail");
        assert!(config_error.to_string().contains("config"));

        let env_error = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: None,
            env_path: Some(invalid.as_path()),
            config_path: None,
            persisted_path: None,
            path_env: None,
            home: Some(temp.path()),
            fallback_env: Vec::new(),
        })
        .expect_err("invalid environment path should fail");
        assert!(env_error.to_string().contains("environment"));
    }

    #[test]
    fn relative_env_and_config_paths_fail_loudly() {
        let cwd = std::env::current_dir().expect("current dir");
        let temp = tempfile::Builder::new()
            .prefix(".codex-cli-relative-configured-")
            .tempdir_in(&cwd)
            .expect("tempdir");
        let cli = temp.path().join("codex");
        write_executable_script(&cli, "#!/bin/sh\necho 'codex-cli v0.42.0'\n").expect("script");
        let relative_cli = cli.strip_prefix(&cwd).expect("relative cli");

        let env_error = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: None,
            env_path: Some(relative_cli),
            config_path: None,
            persisted_path: None,
            path_env: None,
            home: None,
            fallback_env: Vec::new(),
        })
        .expect_err("relative environment path should fail");
        assert!(env_error.to_string().contains("environment"));
        assert!(env_error.to_string().contains("must be absolute"));

        let config_error = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: None,
            env_path: None,
            config_path: Some(relative_cli),
            persisted_path: None,
            path_env: None,
            home: None,
            fallback_env: Vec::new(),
        })
        .expect_err("relative config path should fail");
        assert!(config_error.to_string().contains("config"));
        assert!(config_error.to_string().contains("must be absolute"));
    }

    #[test]
    fn cli_failure_can_preserve_or_clear_resolved_cli_metadata() -> Result<()> {
        let temp = tempdir()?;
        let paths = test_runtime_paths(temp.path());
        paths.ensure_dirs()?;
        let cli_path = temp.path().join("codex");

        let mut state = PersistedState::new(true);
        state.cli_path = Some(cli_path.clone());
        state.cli_path_source = CliPathSource::Persisted;
        state.cli_installed_version = Some("0.42.0".to_string());
        persist_cli_failure(&paths, &mut state, "upgrade failed".to_string(), false)?;
        assert_eq!(state.cli_path.as_deref(), Some(cli_path.as_path()));
        assert_eq!(state.cli_path_source, CliPathSource::Persisted);
        assert_eq!(state.cli_installed_version.as_deref(), Some("0.42.0"));
        assert_eq!(state.cli_status, CliStatus::Failed);

        persist_cli_failure(&paths, &mut state, "missing CLI".to_string(), true)?;
        assert_eq!(state.cli_path, None);
        assert_eq!(state.cli_path_source, CliPathSource::Unknown);
        assert_eq!(state.cli_installed_version, None);
        assert_eq!(state.cli_status, CliStatus::Failed);
        Ok(())
    }

    #[test]
    fn resolves_environment_specific_and_default_fallback_paths() -> Result<()> {
        let temp = tempdir()?;
        let pnpm_cli = temp.path().join("pnpm-home/codex");
        fs::create_dir_all(pnpm_cli.parent().expect("parent"))?;
        write_executable_script(&pnpm_cli, "#!/bin/sh\necho 'codex-cli v0.42.0'\n")?;

        let outcome = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: None,
            env_path: None,
            config_path: None,
            persisted_path: None,
            path_env: None,
            home: Some(temp.path()),
            fallback_env: vec![("PNPM_HOME".into(), temp.path().join("pnpm-home").into())],
        })?
        .expect("Codex CLI path");
        assert_eq!(outcome.path, pnpm_cli);
        assert_eq!(outcome.source, CliPathSource::KnownPath);

        let default_cli = temp.path().join(".local/bin/codex");
        fs::create_dir_all(default_cli.parent().expect("parent"))?;
        write_executable_script(&default_cli, "#!/bin/sh\necho 'codex-cli v0.42.0'\n")?;
        let outcome = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: None,
            env_path: None,
            config_path: None,
            persisted_path: None,
            path_env: None,
            home: Some(temp.path()),
            fallback_env: Vec::new(),
        })?
        .expect("Codex CLI path");
        assert_eq!(outcome.path, default_cli);
        assert_eq!(outcome.source, CliPathSource::KnownPath);
        Ok(())
    }

    #[test]
    fn ignores_relative_fallback_roots() -> Result<()> {
        let temp = tempdir()?;
        let default_cli = temp.path().join(".local/bin/codex");
        fs::create_dir_all(default_cli.parent().expect("parent"))?;
        write_executable_script(&default_cli, "#!/bin/sh\necho 'codex-cli v0.42.0'\n")?;

        let outcome = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: None,
            env_path: None,
            config_path: None,
            persisted_path: None,
            path_env: None,
            home: Some(temp.path()),
            fallback_env: vec![("PNPM_HOME".into(), PathBuf::from("relative-pnpm-home"))],
        })?
        .expect("Codex CLI path");

        assert_eq!(outcome.path, default_cli);
        assert_eq!(outcome.source, CliPathSource::KnownPath);
        Ok(())
    }

    #[test]
    fn ignores_relative_path_entries() -> Result<()> {
        let cwd = std::env::current_dir()?;
        let temp = tempfile::Builder::new()
            .prefix(".codex-cli-relative-path-")
            .tempdir_in(&cwd)?;
        let relative_dir = temp.path().join("relative-bin");
        let absolute_dir = temp.path().join("absolute-bin");
        let relative_cli = relative_dir.join("codex");
        let absolute_cli = absolute_dir.join("codex");
        fs::create_dir_all(&relative_dir)?;
        fs::create_dir_all(&absolute_dir)?;
        write_executable_script(&relative_cli, "#!/bin/sh\necho 'codex-cli v0.41.0'\n")?;
        write_executable_script(&absolute_cli, "#!/bin/sh\necho 'codex-cli v0.42.0'\n")?;
        let relative_dir = relative_dir.strip_prefix(&cwd)?;
        let path_env = std::env::join_paths([relative_dir, absolute_dir.as_path()])?;

        assert_eq!(find_in_path("codex", &path_env), Some(absolute_cli));
        Ok(())
    }

    #[test]
    fn absolutizes_resolved_cli_paths_without_canonicalizing() -> Result<()> {
        let cwd = std::env::current_dir()?;
        let temp = tempfile::Builder::new()
            .prefix(".codex-cli-absolute-")
            .tempdir_in(&cwd)?;
        let cli = temp.path().join("codex");
        write_executable_script(&cli, "#!/bin/sh\necho 'codex-cli v0.42.0'\n")?;
        let relative_cli = cli.strip_prefix(&cwd)?;

        let outcome = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: Some(relative_cli),
            env_path: None,
            config_path: None,
            persisted_path: None,
            path_env: None,
            home: None,
            fallback_env: Vec::new(),
        })?
        .expect("Codex CLI path");

        assert!(outcome.path.is_absolute());
        assert_eq!(outcome.path, cwd.join(relative_cli));
        assert_eq!(outcome.source, CliPathSource::Explicit);
        Ok(())
    }

    #[test]
    fn preserves_symlinked_cli_paths() -> Result<()> {
        let temp = tempdir()?;
        let target = temp.path().join("codex-v0.42.0");
        let shim = temp.path().join("codex");
        write_executable_script(&target, "#!/bin/sh\necho 'codex-cli v0.42.0'\n")?;
        std::os::unix::fs::symlink(&target, &shim)?;

        let outcome = resolve_cli_path(&ResolveCliPathRequest {
            explicit_path: Some(shim.as_path()),
            env_path: None,
            config_path: None,
            persisted_path: None,
            path_env: None,
            home: None,
            fallback_env: Vec::new(),
        })?
        .expect("Codex CLI path");

        assert_eq!(outcome.path, shim);
        assert_ne!(outcome.path, fs::canonicalize(&target)?);
        assert_eq!(outcome.source, CliPathSource::Explicit);
        Ok(())
    }
}
