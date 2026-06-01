//! Runtime configuration loading and XDG path discovery for the updater.

use anyhow::{Context, Result};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

const SERVICE_NAME: &str = "codex-app-updater";
pub const PACKAGED_BUILDER_BUNDLE_ROOT: &str = "/usr/lib/codex-app/update-builder";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Runtime configuration values that control how the updater behaves on Linux.
pub struct RuntimeConfig {
    pub dmg_url: String,
    pub initial_check_delay_seconds: u64,
    pub check_interval_hours: u64,
    pub auto_install_on_app_exit: bool,
    pub notifications: bool,
    #[serde(default)]
    pub developer_mode: bool,
    pub workspace_root: PathBuf,
    pub builder_bundle_root: PathBuf,
    pub app_executable_path: PathBuf,
    #[serde(default)]
    pub cli_path: Option<PathBuf>,
    /// Opt-in tracking of newer codex-app wrapper releases in addition to the
    /// official Codex DMG. Off by default so existing installs keep DMG-only
    /// behavior.
    #[serde(default)]
    pub enable_wrapper_updates: bool,
    /// Git remote (name or URL) used to detect wrapper updates. Empty means
    /// use the builder checkout's configured origin.
    #[serde(default)]
    pub wrapper_remote: String,
    /// Branch to track for wrapper updates.
    #[serde(default = "default_wrapper_branch")]
    pub wrapper_branch: String,
}

fn default_wrapper_branch() -> String {
    "main".to_string()
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RuntimeConfigOverlay {
    dmg_url: Option<String>,
    initial_check_delay_seconds: Option<u64>,
    check_interval_hours: Option<u64>,
    auto_install_on_app_exit: Option<bool>,
    notifications: Option<bool>,
    developer_mode: Option<bool>,
    workspace_root: Option<PathBuf>,
    builder_bundle_root: Option<PathBuf>,
    app_executable_path: Option<PathBuf>,
    cli_path: Option<PathBuf>,
    enable_wrapper_updates: Option<bool>,
    wrapper_remote: Option<String>,
    wrapper_branch: Option<String>,
}

#[derive(Debug, Clone)]
/// Resolved XDG filesystem locations used by the updater at runtime.
pub struct RuntimePaths {
    pub config_file: PathBuf,
    pub state_file: PathBuf,
    pub log_file: PathBuf,
    pub cache_dir: PathBuf,
    pub state_dir: PathBuf,
    pub config_dir: PathBuf,
}

impl RuntimePaths {
    /// Resolves updater paths from the current user's XDG base directories.
    pub fn from_base_dirs(base_dirs: &BaseDirs) -> Self {
        let config_dir = base_dirs.config_dir().join(SERVICE_NAME);
        let state_root = base_dirs
            .state_dir()
            .unwrap_or_else(|| base_dirs.data_local_dir());
        let state_dir = state_root.join(SERVICE_NAME);
        let cache_dir = base_dirs.cache_dir().join(SERVICE_NAME);

        Self {
            config_file: config_dir.join("config.toml"),
            state_file: state_dir.join("state.json"),
            log_file: state_dir.join("service.log"),
            cache_dir,
            state_dir,
            config_dir,
        }
    }

    /// Detects updater paths for the current machine.
    pub fn detect() -> Result<Self> {
        let base_dirs = BaseDirs::new().context("Could not resolve XDG base directories")?;
        Ok(Self::from_base_dirs(&base_dirs))
    }

    /// Creates the runtime directories needed by the updater.
    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.config_dir)
            .with_context(|| format!("Failed to create {}", self.config_dir.display()))?;
        fs::create_dir_all(&self.state_dir)
            .with_context(|| format!("Failed to create {}", self.state_dir.display()))?;
        fs::create_dir_all(&self.cache_dir)
            .with_context(|| format!("Failed to create {}", self.cache_dir.display()))?;
        Ok(())
    }
}

impl RuntimeConfig {
    /// Builds the default runtime configuration for the resolved paths.
    pub fn default_with_paths(paths: &RuntimePaths) -> Self {
        let packaged_bundle_root = PathBuf::from(PACKAGED_BUILDER_BUNDLE_ROOT);
        let builder_bundle_root = if packaged_bundle_root.exists() {
            packaged_bundle_root
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("updater crate should live inside the repository root")
                .to_path_buf()
        };

        Self {
            dmg_url: "https://persistent.oaistatic.com/codex-app-prod/Codex.dmg".to_string(),
            initial_check_delay_seconds: 30,
            check_interval_hours: 6,
            auto_install_on_app_exit: true,
            notifications: true,
            developer_mode: false,
            workspace_root: paths.cache_dir.clone(),
            builder_bundle_root,
            app_executable_path: PathBuf::from("/opt/codex-app/electron"),
            cli_path: None,
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: default_wrapper_branch(),
        }
    }

    /// Loads the runtime configuration from disk, or returns defaults if missing.
    pub fn load_or_default(paths: &RuntimePaths) -> Result<Self> {
        if !paths.config_file.exists() {
            return Ok(Self::default_with_paths(paths));
        }

        let content = fs::read_to_string(&paths.config_file)
            .with_context(|| format!("Failed to read {}", paths.config_file.display()))?;
        let overlay = toml::from_str::<RuntimeConfigOverlay>(&content)
            .with_context(|| format!("Failed to parse {}", paths.config_file.display()))?;
        let mut config = Self::default_with_paths(paths);
        config.apply_overlay(overlay);
        config.enforce_packaged_builder_root(Path::new(PACKAGED_BUILDER_BUNDLE_ROOT));
        Ok(config)
    }

    fn apply_overlay(&mut self, overlay: RuntimeConfigOverlay) {
        if let Some(value) = overlay.dmg_url {
            self.dmg_url = value;
        }
        if let Some(value) = overlay.initial_check_delay_seconds {
            self.initial_check_delay_seconds = value;
        }
        if let Some(value) = overlay.check_interval_hours {
            self.check_interval_hours = value;
        }
        if let Some(value) = overlay.auto_install_on_app_exit {
            self.auto_install_on_app_exit = value;
        }
        if let Some(value) = overlay.notifications {
            self.notifications = value;
        }
        if let Some(value) = overlay.developer_mode {
            self.developer_mode = value;
        }
        if let Some(value) = overlay.workspace_root {
            self.workspace_root = value;
        }
        if let Some(value) = overlay.builder_bundle_root {
            self.builder_bundle_root = value;
        }
        if let Some(value) = overlay.app_executable_path {
            self.app_executable_path = value;
        }
        if let Some(value) = overlay.cli_path {
            self.cli_path = Some(value);
        }
        if let Some(value) = overlay.enable_wrapper_updates {
            self.enable_wrapper_updates = value;
        }
        if let Some(value) = overlay.wrapper_remote {
            self.wrapper_remote = value;
        }
        if let Some(value) = overlay.wrapper_branch {
            self.wrapper_branch = value;
        }
    }

    fn enforce_packaged_builder_root(&mut self, packaged_root: &Path) {
        if packaged_root.exists() && !self.developer_mode {
            self.builder_bundle_root = packaged_root.to_path_buf();
        }
    }
}

const APP_SETTINGS_FILE: &str = "settings.json";
const DEFAULT_APP_ID: &str = "codex-app";
const AUTO_INSTALL_SETTING_KEY: &str = "codex-linux-auto-update-on-exit";
const WRAPPER_UPDATES_SETTING_KEY: &str = "codex-linux-wrapper-updates-enabled";

/// Resolves the Codex App id the same way the Linux launcher and main bundle do:
/// `CODEX_LINUX_APP_ID`, then `CODEX_APP_ID`, then `codex-app`.
/// Invalid ids fall back to the default so a malformed env value can never point
/// the lookup at an attacker-controlled path.
fn resolve_app_id() -> String {
    fn valid(id: &str) -> bool {
        !id.is_empty()
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    }

    for var in ["CODEX_LINUX_APP_ID", "CODEX_APP_ID"] {
        if let Ok(value) = std::env::var(var) {
            if valid(&value) {
                return value;
            }
        }
    }
    DEFAULT_APP_ID.to_string()
}

/// Resolves the app `settings.json` path mirroring the launcher
/// (`launcher/start.sh.template`) and the main-bundle persistence helper
/// (`scripts/patches/launch-actions.js`): honor `CODEX_LINUX_SETTINGS_FILE`
/// first, then `XDG_CONFIG_HOME`, then `$HOME/.config`, joined with the app id.
fn app_settings_path() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("CODEX_LINUX_SETTINGS_FILE") {
        if !explicit.is_empty() {
            return Some(PathBuf::from(explicit));
        }
    }

    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|home| !home.as_os_str().is_empty())
                .map(|home| home.join(".config"))
        })?;

    Some(config_home.join(resolve_app_id()).join(APP_SETTINGS_FILE))
}

/// Coerces a settings.json value into a boolean the same way the launcher's
/// `linux_setting_enabled` helper does: real booleans pass through, numbers are
/// truthy when non-zero, and strings are falsey only for `0/false/no/off`.
fn coerce_setting_bool(value: &serde_json::Value) -> Option<bool> {
    match value {
        serde_json::Value::Bool(flag) => Some(*flag),
        serde_json::Value::Number(number) => number.as_f64().map(|n| n != 0.0),
        serde_json::Value::String(text) => {
            let normalized = text.trim().to_ascii_lowercase();
            Some(!matches!(normalized.as_str(), "0" | "false" | "no" | "off"))
        }
        _ => None,
    }
}

/// Reads a boolean app setting from `settings.json`. Returns `Some(true|false)`
/// only when the toggle key is present and coercible; any missing file, parse
/// error, or absent key yields `None` so callers fall back to config/defaults.
fn settings_bool_override(key: &str) -> Option<bool> {
    let path = app_settings_path()?;
    let content = fs::read_to_string(&path).ok()?;
    let parsed = serde_json::from_str::<serde_json::Value>(&content).ok()?;
    let object = parsed.as_object()?;
    coerce_setting_bool(object.get(key)?)
}

/// Reads the user's auto-install-on-exit preference from the app settings.
pub fn settings_auto_install_override() -> Option<bool> {
    settings_bool_override(AUTO_INSTALL_SETTING_KEY)
}

/// Reads the user's opt-in wrapper update tracking preference from app settings.
pub fn settings_wrapper_updates_override() -> Option<bool> {
    settings_bool_override(WRAPPER_UPDATES_SETTING_KEY)
}

const FEATURE_CONFIG_FILE: &str = "port-integrations.json";
const PACKAGED_FEATURE_CONFIG_DIR: &str = ".codex-linux";
const BUNDLED_FEATURE_CONFIG_FILE: &str = "integrations.json";
const FEATURE_PICKER_ON_UPDATE_SETTING_KEY: &str = "codex-linux-integration-picker-on-update";

/// Resolves the stable per-user port-integration config path
/// (`<config>/<appId>/port-integrations.json`), alongside `settings.json`. The
/// wrapper-update picker writes the chosen `{"enabled":[...]}` here, and the
/// rebuild points `CODEX_PORT_INTEGRATIONS_CONFIG` at it. Deliberately outside
/// any wrapper-src checkout so a fresh clone cannot clobber it.
pub fn integration_config_path() -> Option<PathBuf> {
    let settings = app_settings_path()?;
    let dir = settings.parent()?;
    Some(dir.join(FEATURE_CONFIG_FILE))
}

/// Returns the port-integration config that should drive a rebuild. A saved
/// per-user picker selection wins; otherwise preserve the currently
/// installed/bundled integration selection from the builder bundle.
pub fn effective_integration_config_path(config: &RuntimeConfig) -> Option<PathBuf> {
    integration_config_path()
        .filter(|path| path.is_file())
        .or_else(|| {
            let packaged = config
                .builder_bundle_root
                .join(PACKAGED_FEATURE_CONFIG_DIR)
                .join(FEATURE_CONFIG_FILE);
            packaged.is_file().then_some(packaged)
        })
        .or_else(|| {
            let bundled = config
                .builder_bundle_root
                .join("port-integrations")
                .join(BUNDLED_FEATURE_CONFIG_FILE);
            bundled.is_file().then_some(bundled)
        })
}

/// Reads the user's "ask which integrations to enable on update" preference.
/// Absent means callers use their default.
pub fn settings_integration_picker_on_update_override() -> Option<bool> {
    settings_bool_override(FEATURE_PICKER_ON_UPDATE_SETTING_KEY)
}

/// Persists the "Ask which integrations to enable on update" preference to the app
/// `settings.json`, merging into the existing object (preserving every other
/// key). Used to honor the picker's "Don't ask again" row. Never panics; returns
/// the IO/serialization error so the caller can log-and-continue.
pub fn write_integration_picker_on_update(value: bool) -> Result<()> {
    write_settings_bool(FEATURE_PICKER_ON_UPDATE_SETTING_KEY, value)
}

/// Read-modify-writes a boolean key into the app `settings.json`, preserving all
/// other keys. Creates the file (and parent dir) when absent. A malformed
/// existing file is replaced with a fresh object rather than failing.
fn write_settings_bool(key: &str, value: bool) -> Result<()> {
    let path = app_settings_path().context("could not resolve settings.json path")?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    }
    let mut object = fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    object.insert(key.to_string(), serde_json::Value::Bool(value));
    let serialized = serde_json::to_string_pretty(&serde_json::Value::Object(object))
        .context("Failed to serialize settings.json")?;
    atomic_write(&path, format!("{serialized}\n").as_bytes())?;
    Ok(())
}

pub(crate) fn atomic_write(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("{} has no parent directory", path.display()))?;
    fs::create_dir_all(parent).with_context(|| format!("Failed to create {}", parent.display()))?;

    let temp_path = atomic_temp_path(path);
    let mut temp_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .with_context(|| format!("Failed to create {}", temp_path.display()))?;

    let write_result = (|| -> Result<()> {
        temp_file
            .write_all(contents)
            .with_context(|| format!("Failed to write {}", temp_path.display()))?;
        temp_file
            .sync_all()
            .with_context(|| format!("Failed to sync {}", temp_path.display()))?;
        Ok(())
    })();

    if let Err(error) = write_result {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }

    fs::rename(&temp_path, path).with_context(|| {
        format!(
            "Failed to atomically replace {} with {}",
            path.display(),
            temp_path.display()
        )
    })?;
    Ok(())
}

fn atomic_temp_path(path: &Path) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("settings.json");
    path.with_file_name(format!(".{file_name}.tmp.{}.{}", process::id(), timestamp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use tempfile::tempdir;

    /// Writes `settings.json` content to a tempfile, points
    /// `CODEX_LINUX_SETTINGS_FILE` at it, and returns the override result.
    /// `None` content means "do not create the file" (missing-file case).
    fn override_with_settings(content: Option<&str>, key: &str) -> Option<bool> {
        let _guard = crate::test_util::env_lock();
        let temp = tempdir().expect("tempdir");
        let settings_path = temp.path().join("settings.json");
        if let Some(body) = content {
            std::fs::write(&settings_path, body).expect("write settings");
        }
        std::env::set_var("CODEX_LINUX_SETTINGS_FILE", &settings_path);
        let result = settings_bool_override(key);
        std::env::remove_var("CODEX_LINUX_SETTINGS_FILE");
        result
    }

    #[test]
    fn settings_override_reads_explicit_bool() {
        assert_eq!(
            override_with_settings(
                Some(r#"{"codex-linux-auto-update-on-exit": false}"#),
                AUTO_INSTALL_SETTING_KEY
            ),
            Some(false)
        );
        assert_eq!(
            override_with_settings(
                Some(r#"{"codex-linux-auto-update-on-exit": true}"#),
                AUTO_INSTALL_SETTING_KEY
            ),
            Some(true)
        );
    }

    #[test]
    fn settings_override_coerces_string_and_number() {
        assert_eq!(
            override_with_settings(
                Some(r#"{"codex-linux-auto-update-on-exit": "off"}"#),
                AUTO_INSTALL_SETTING_KEY
            ),
            Some(false)
        );
        assert_eq!(
            override_with_settings(
                Some(r#"{"codex-linux-auto-update-on-exit": "on"}"#),
                AUTO_INSTALL_SETTING_KEY
            ),
            Some(true)
        );
        assert_eq!(
            override_with_settings(
                Some(r#"{"codex-linux-auto-update-on-exit": 0}"#),
                AUTO_INSTALL_SETTING_KEY
            ),
            Some(false)
        );
        assert_eq!(
            override_with_settings(
                Some(r#"{"codex-linux-auto-update-on-exit": 1}"#),
                AUTO_INSTALL_SETTING_KEY
            ),
            Some(true)
        );
    }

    #[test]
    fn settings_override_absent_yields_none() {
        // Missing file, malformed JSON, non-object, and absent key all fall back.
        assert_eq!(override_with_settings(None, AUTO_INSTALL_SETTING_KEY), None);
        assert_eq!(
            override_with_settings(Some("not json{"), AUTO_INSTALL_SETTING_KEY),
            None
        );
        assert_eq!(
            override_with_settings(Some("[1,2,3]"), AUTO_INSTALL_SETTING_KEY),
            None
        );
        assert_eq!(
            override_with_settings(Some(r#"{"other-key": true}"#), AUTO_INSTALL_SETTING_KEY),
            None
        );
    }

    #[test]
    fn wrapper_settings_override_reads_explicit_bool() {
        assert_eq!(
            override_with_settings(
                Some(r#"{"codex-linux-wrapper-updates-enabled": true}"#),
                WRAPPER_UPDATES_SETTING_KEY
            ),
            Some(true)
        );
        assert_eq!(
            override_with_settings(
                Some(r#"{"codex-linux-wrapper-updates-enabled": false}"#),
                WRAPPER_UPDATES_SETTING_KEY
            ),
            Some(false)
        );
    }

    #[test]
    fn writes_integration_picker_setting_without_clobbering_other_settings() -> Result<()> {
        let _guard = crate::test_util::env_lock();
        let temp = tempdir()?;
        let settings_path = temp.path().join("settings.json");
        fs::write(&settings_path, r#"{"theme":"dark"}"#)?;
        let _settings_guard = crate::test_util::EnvVarGuard::set(
            &_guard,
            "CODEX_LINUX_SETTINGS_FILE",
            &settings_path,
        );

        write_integration_picker_on_update(false)?;

        let settings = fs::read_to_string(&settings_path)?;
        let value = serde_json::from_str::<serde_json::Value>(&settings)?;
        assert_eq!(value["theme"], serde_json::Value::String("dark".into()));
        assert_eq!(
            value["codex-linux-integration-picker-on-update"],
            serde_json::Value::Bool(false)
        );
        let temp_entries = fs::read_dir(temp.path())?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".settings.json.tmp.")
            })
            .count();
        assert_eq!(temp_entries, 0);

        Ok(())
    }

    #[test]
    fn effective_integration_config_prefers_saved_picker_then_packaged_then_legacy_builder_config(
    ) -> Result<()> {
        let _guard = crate::test_util::env_lock();
        let temp = tempdir()?;
        let settings_dir = temp.path().join("settings");
        let settings_file = settings_dir.join("settings.json");
        let saved_integration_config = settings_dir.join("port-integrations.json");
        let packaged_integration_config = temp
            .path()
            .join("builder/.codex-linux/port-integrations.json");
        let builder_integration_config = temp
            .path()
            .join("builder/port-integrations/integrations.json");

        fs::create_dir_all(builder_integration_config.parent().unwrap())?;
        fs::create_dir_all(packaged_integration_config.parent().unwrap())?;
        fs::write(
            &packaged_integration_config,
            r#"{"enabled":["conversation-mode"],"disabled":["open-target-discovery"]}"#,
        )?;
        fs::write(
            &builder_integration_config,
            r#"{"enabled":["codex-wrapper-updater"]}"#,
        )?;
        std::env::set_var("CODEX_LINUX_SETTINGS_FILE", &settings_file);

        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        let mut config = RuntimeConfig::default_with_paths(&paths);
        config.builder_bundle_root = temp.path().join("builder");

        assert_eq!(
            effective_integration_config_path(&config),
            Some(packaged_integration_config.clone())
        );

        fs::remove_file(&packaged_integration_config)?;
        assert_eq!(
            effective_integration_config_path(&config),
            Some(builder_integration_config)
        );

        fs::write(
            &packaged_integration_config,
            r#"{"enabled":["conversation-mode"],"disabled":["open-target-discovery"]}"#,
        )?;

        fs::create_dir_all(&settings_dir)?;
        fs::write(&saved_integration_config, r#"{"enabled":["read-aloud"]}"#)?;
        assert_eq!(
            effective_integration_config_path(&config),
            Some(saved_integration_config)
        );

        std::env::remove_var("CODEX_LINUX_SETTINGS_FILE");
        Ok(())
    }

    #[test]
    fn loads_default_when_config_is_missing() -> Result<()> {
        let temp = tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };

        let config = RuntimeConfig::load_or_default(&paths)?;
        assert_eq!(config.initial_check_delay_seconds, 30);
        assert!(config.auto_install_on_app_exit);
        assert_eq!(config.workspace_root, paths.cache_dir);
        assert!(config.builder_bundle_root.is_absolute());
        Ok(())
    }

    #[test]
    fn parses_runtime_config_from_disk() -> Result<()> {
        let temp = tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        fs::create_dir_all(&paths.config_dir)?;
        fs::write(
            &paths.config_file,
            r#"
dmg_url = "https://example.com/Codex.dmg"
initial_check_delay_seconds = 5
check_interval_hours = 12
auto_install_on_app_exit = false
notifications = false
developer_mode = true
workspace_root = "/tmp/codex-workspaces"
builder_bundle_root = "/tmp/codex-builder"
app_executable_path = "/opt/codex-app/electron"
cli_path = "/opt/codex/bin/codex"
"#,
        )?;

        let config = RuntimeConfig::load_or_default(&paths)?;
        assert_eq!(config.dmg_url, "https://example.com/Codex.dmg");
        assert_eq!(config.initial_check_delay_seconds, 5);
        assert_eq!(config.check_interval_hours, 12);
        assert!(!config.auto_install_on_app_exit);
        assert!(!config.notifications);
        assert!(config.developer_mode);
        assert_eq!(
            config.workspace_root,
            PathBuf::from("/tmp/codex-workspaces")
        );
        assert_eq!(
            config.builder_bundle_root,
            PathBuf::from("/tmp/codex-builder")
        );
        assert_eq!(
            config.app_executable_path,
            PathBuf::from("/opt/codex-app/electron")
        );
        assert_eq!(config.cli_path, Some(PathBuf::from("/opt/codex/bin/codex")));
        Ok(())
    }

    #[test]
    fn loads_cli_path_only_config_as_default_overlay() -> Result<()> {
        let temp = tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        fs::create_dir_all(&paths.config_dir)?;
        fs::write(&paths.config_file, r#"cli_path = "/opt/codex/bin/codex""#)?;

        let config = RuntimeConfig::load_or_default(&paths)?;
        assert_eq!(config.initial_check_delay_seconds, 30);
        assert!(config.auto_install_on_app_exit);
        assert_eq!(config.workspace_root, paths.cache_dir);
        assert_eq!(config.cli_path, Some(PathBuf::from("/opt/codex/bin/codex")));
        Ok(())
    }

    #[test]
    fn rejects_unknown_config_keys() -> Result<()> {
        let temp = tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        fs::create_dir_all(&paths.config_dir)?;
        fs::write(&paths.config_file, r#"cli_pth = "/opt/codex/bin/codex""#)?;

        let error = RuntimeConfig::load_or_default(&paths).expect_err("unknown key should fail");
        assert!(error.to_string().contains("Failed to parse"));
        Ok(())
    }

    #[test]
    fn packaged_builder_root_overrides_configured_root_without_developer_mode() {
        let temp = tempdir().expect("tempdir");
        let packaged_root = temp.path().join("usr/lib/codex-app/update-builder");
        fs::create_dir_all(&packaged_root).expect("packaged root");
        let configured_root = temp.path().join("custom-builder");
        let mut config = RuntimeConfig {
            dmg_url: "https://example.com/Codex.dmg".to_string(),
            initial_check_delay_seconds: 5,
            check_interval_hours: 12,
            auto_install_on_app_exit: false,
            notifications: false,
            developer_mode: false,
            workspace_root: temp.path().join("workspace"),
            builder_bundle_root: configured_root,
            app_executable_path: PathBuf::from("/opt/codex-app/electron"),
            cli_path: None,
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        config.enforce_packaged_builder_root(&packaged_root);

        assert_eq!(config.builder_bundle_root, packaged_root);
    }

    #[test]
    fn developer_mode_preserves_configured_builder_root() {
        let temp = tempdir().expect("tempdir");
        let packaged_root = temp.path().join("usr/lib/codex-app/update-builder");
        fs::create_dir_all(&packaged_root).expect("packaged root");
        let configured_root = temp.path().join("custom-builder");
        let mut config = RuntimeConfig {
            dmg_url: "https://example.com/Codex.dmg".to_string(),
            initial_check_delay_seconds: 5,
            check_interval_hours: 12,
            auto_install_on_app_exit: false,
            notifications: false,
            developer_mode: true,
            workspace_root: temp.path().join("workspace"),
            builder_bundle_root: configured_root.clone(),
            app_executable_path: PathBuf::from("/opt/codex-app/electron"),
            cli_path: None,
            enable_wrapper_updates: false,
            wrapper_remote: String::new(),
            wrapper_branch: "main".to_string(),
        };

        config.enforce_packaged_builder_root(&packaged_root);

        assert_eq!(config.builder_bundle_root, configured_root);
    }

    #[test]
    fn merges_partial_runtime_config_with_defaults() -> Result<()> {
        let temp = tempdir()?;
        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: temp.path().join("state/state.json"),
            log_file: temp.path().join("state/service.log"),
            cache_dir: temp.path().join("cache"),
            state_dir: temp.path().join("state"),
            config_dir: temp.path().join("config"),
        };
        fs::create_dir_all(&paths.config_dir)?;
        fs::write(
            &paths.config_file,
            r#"
dmg_url = "https://example.com/Codex.dmg"
notifications = false
"#,
        )?;

        let config = RuntimeConfig::load_or_default(&paths)?;
        assert_eq!(config.dmg_url, "https://example.com/Codex.dmg");
        assert_eq!(config.initial_check_delay_seconds, 30);
        assert_eq!(config.check_interval_hours, 6);
        assert!(config.auto_install_on_app_exit);
        assert!(!config.notifications);
        assert_eq!(config.workspace_root, paths.cache_dir);
        Ok(())
    }
}
