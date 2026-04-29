//! Rebuilds native Linux packages from a downloaded upstream DMG.

use crate::{
    config::{RuntimeConfig, RuntimePaths, PACKAGED_BUILDER_BUNDLE_ROOT},
    install::PackageKind,
    package_version,
    state::{ArtifactPaths, PersistedState, UpdateStatus},
};
use anyhow::{Context, Result};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};
use tokio::process::Command;
use tracing::info;

const REQUIRED_BUNDLE_FILES: [(&str, &str); 6] = [
    ("install.sh", "install.sh"),
    ("scripts/build-deb.sh", "scripts/build-deb.sh"),
    (
        "scripts/patch-linux-window-ui.js",
        "scripts/patch-linux-window-ui.js",
    ),
    (
        "scripts/lib/package-common.sh",
        "scripts/lib/package-common.sh",
    ),
    ("packaging/linux", "packaging/linux"),
    ("assets/codex.png", "assets/codex.png"),
];
const OPTIONAL_BUNDLE_FILES: [(&str, &str); 2] = [
    ("scripts/build-rpm.sh", "scripts/build-rpm.sh"),
    ("scripts/build-pacman.sh", "scripts/build-pacman.sh"),
];
const PACMAN_PACKAGE_SUFFIXES: &[&str] = &[
    ".pkg.tar.zst",
    ".pkg.tar.xz",
    ".pkg.tar.gz",
    ".pkg.tar.bz2",
    ".pkg.tar.lz",
    ".pkg.tar.lz4",
    ".pkg.tar.lz5",
];
const WORKSPACE_ID_PATH_LEN: usize = 16;
const BUILD_COMMAND_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/bin:/bin";

#[derive(Debug, Clone, PartialEq, Eq)]
/// Paths to the temporary workspace and generated package produced by a rebuild.
pub struct BuildArtifacts {
    pub workspace_dir: PathBuf,
    pub package_path: PathBuf,
}

/// Rebuilds a Linux package from the downloaded upstream DMG.
pub async fn build_update(
    config: &RuntimeConfig,
    state: &mut PersistedState,
    paths: &RuntimePaths,
    workspace_id: &str,
    dmg_path: &Path,
) -> Result<Option<BuildArtifacts>> {
    let workspace = BuilderWorkspace::prepare(&config.workspace_root, workspace_id)?;
    let build_path = build_command_path();

    state.status = UpdateStatus::PreparingWorkspace;
    state.artifact_paths.workspace_dir = Some(workspace.workspace_dir.clone());
    state.save(&paths.state_file)?;

    copy_builder_bundle(
        &config.builder_bundle_root,
        &workspace.bundle_dir,
        config.developer_mode,
    )?;

    state.status = UpdateStatus::PatchingApp;
    state.save(&paths.state_file)?;
    run_and_log(
        Command::new(workspace.bundle_dir.join("install.sh"))
            .arg(dmg_path)
            .env("CODEX_INSTALL_DIR", &workspace.app_dir)
            .env("PATH", &build_path)
            .current_dir(&workspace.bundle_dir),
        &workspace.install_log,
    )
    .await
    .context("install.sh failed during local rebuild")?;

    let package_version = app_package_version(&workspace.app_dir)?;
    if package_version::installed_version_satisfies_candidate(
        &state.installed_version,
        &package_version,
    ) {
        state.status = UpdateStatus::Idle;
        state.candidate_version = None;
        state.artifact_paths = ArtifactPaths {
            dmg_path: Some(dmg_path.to_path_buf()),
            workspace_dir: Some(workspace.workspace_dir.clone()),
            package_path: None,
        };
        state.save(&paths.state_file)?;
        info!(candidate_version = %package_version, installed_version = %state.installed_version, "upstream app version is already installed; skipping package rebuild");
        return Ok(None);
    }
    state.candidate_version = Some(package_version.clone());

    state.status = UpdateStatus::BuildingPackage;
    state.save(&paths.state_file)?;

    let build_script = package_build_script(&workspace.bundle_dir);
    run_and_log(
        Command::new(&build_script)
            .env("PACKAGE_VERSION", &package_version)
            .env("APP_DIR_OVERRIDE", &workspace.app_dir)
            .env("DIST_DIR_OVERRIDE", &workspace.dist_dir)
            .env("UPDATER_BINARY_SOURCE", std::env::current_exe()?)
            .env(
                "UPDATER_SERVICE_SOURCE",
                workspace
                    .bundle_dir
                    .join("packaging/linux/codex-app-updater.service"),
            )
            .env("PATH", &build_path)
            .current_dir(&workspace.bundle_dir),
        &workspace.build_log,
    )
    .await
    .with_context(|| format!("{} failed during local rebuild", build_script.display()))?;

    let package_path = find_package_in(&workspace.dist_dir)?;
    state.status = UpdateStatus::ReadyToInstall;
    state.artifact_paths = ArtifactPaths {
        dmg_path: Some(dmg_path.to_path_buf()),
        workspace_dir: Some(workspace.workspace_dir.clone()),
        package_path: Some(package_path.clone()),
    };
    state.save(&paths.state_file)?;
    info!(candidate_version = %package_version, package = %package_path.display(), "local update build ready");

    Ok(Some(BuildArtifacts {
        workspace_dir: workspace.workspace_dir,
        package_path,
    }))
}

fn app_package_version(app_dir: &Path) -> Result<String> {
    let metadata_path = app_dir.join("codex-app-version.env");
    let metadata = fs::read_to_string(&metadata_path)
        .with_context(|| format!("Failed to read {}", metadata_path.display()))?;

    let version = metadata
        .lines()
        .find_map(|line| line.strip_prefix("CODEX_APP_PACKAGE_VERSION="))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("Missing CODEX_APP_PACKAGE_VERSION in generated app metadata")?;

    let version_parts: Vec<_> = version.split('.').collect();
    anyhow::ensure!(
        (3..=4).contains(&version_parts.len())
            && version_parts
                .iter()
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit())),
        "Invalid CODEX_APP_PACKAGE_VERSION in generated app metadata: {version}"
    );

    Ok(version.to_string())
}

#[derive(Debug, Clone)]
struct BuilderWorkspace {
    workspace_dir: PathBuf,
    bundle_dir: PathBuf,
    dist_dir: PathBuf,
    app_dir: PathBuf,
    install_log: PathBuf,
    build_log: PathBuf,
}

impl BuilderWorkspace {
    fn prepare(workspace_root: &Path, workspace_id: &str) -> Result<Self> {
        let workspace_dir = workspace_root
            .join("workspaces")
            .join(workspace_path_component(workspace_id)?);
        let bundle_dir = workspace_dir.join("builder");
        let dist_dir = workspace_dir.join("dist");
        let app_dir = workspace_dir.join("codex-app");
        let logs_dir = workspace_dir.join("logs");
        let install_log = logs_dir.join("install.log");
        let build_log = logs_dir.join("build-package.log");

        if workspace_dir.exists() {
            fs::remove_dir_all(&workspace_dir)
                .with_context(|| format!("Failed to remove {}", workspace_dir.display()))?;
        }

        fs::create_dir_all(&logs_dir)
            .with_context(|| format!("Failed to create {}", logs_dir.display()))?;

        Ok(Self {
            workspace_dir,
            bundle_dir,
            dist_dir,
            app_dir,
            install_log,
            build_log,
        })
    }
}

fn workspace_path_component(workspace_id: &str) -> Result<&str> {
    let workspace_id = workspace_id.trim();
    anyhow::ensure!(!workspace_id.is_empty(), "Workspace id must not be empty");
    anyhow::ensure!(
        workspace_id.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Workspace id must be a hex digest"
    );
    Ok(&workspace_id[..workspace_id.len().min(WORKSPACE_ID_PATH_LEN)])
}

/// Returns the path to the native-package build script appropriate for the running system.
fn package_build_script(bundle_dir: &Path) -> PathBuf {
    match PackageKind::detect() {
        PackageKind::Rpm => bundle_dir.join("scripts/build-rpm.sh"),
        PackageKind::Pacman => bundle_dir.join("scripts/build-pacman.sh"),
        PackageKind::Deb => bundle_dir.join("scripts/build-deb.sh"),
    }
}

fn copy_builder_bundle(
    source_root: &Path,
    destination_root: &Path,
    developer_mode: bool,
) -> Result<()> {
    let validation = BuilderBundleValidation::new(source_root, developer_mode);
    validate_builder_bundle_source(source_root, validation)?;

    for (source, destination) in REQUIRED_BUNDLE_FILES {
        copy_entry(
            &source_root.join(source),
            &destination_root.join(destination),
            false,
            validation,
        )?;
    }

    for (source, destination) in OPTIONAL_BUNDLE_FILES {
        copy_entry(
            &source_root.join(source),
            &destination_root.join(destination),
            true,
            validation,
        )?;
    }

    Ok(())
}

#[derive(Clone, Copy)]
struct BuilderBundleValidation {
    developer_mode: bool,
    require_root_owner: bool,
    kernel_overflow_uid: Option<u32>,
}

impl BuilderBundleValidation {
    fn new(source_root: &Path, developer_mode: bool) -> Self {
        let require_root_owner =
            !developer_mode && source_root == Path::new(PACKAGED_BUILDER_BUNDLE_ROOT);
        let kernel_overflow_uid = require_root_owner.then(kernel_overflow_uid).flatten();

        Self {
            developer_mode,
            require_root_owner,
            kernel_overflow_uid,
        }
    }
}

fn validate_builder_bundle_source(
    source_root: &Path,
    validation: BuilderBundleValidation,
) -> Result<()> {
    let metadata = fs::symlink_metadata(source_root).with_context(|| {
        format!(
            "Failed to stat builder bundle root {}",
            source_root.display()
        )
    })?;
    anyhow::ensure!(
        !metadata.file_type().is_symlink(),
        "Builder bundle root must not be a symlink: {}",
        source_root.display()
    );
    anyhow::ensure!(
        metadata.is_dir(),
        "Builder bundle root must be a directory: {}",
        source_root.display()
    );
    validate_builder_bundle_entry(source_root, &metadata, validation)?;

    Ok(())
}

fn validate_builder_bundle_entry(
    path: &Path,
    metadata: &fs::Metadata,
    validation: BuilderBundleValidation,
) -> Result<()> {
    anyhow::ensure!(
        !metadata.file_type().is_symlink(),
        "Builder bundle path must not be a symlink: {}",
        path.display()
    );
    #[cfg(unix)]
    if !validation.developer_mode {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let mode = metadata.permissions().mode();
        anyhow::ensure!(
            mode & 0o022 == 0,
            "Builder bundle path must not be group- or world-writable: {}",
            path.display()
        );

        if validation.require_root_owner {
            let uid = metadata.uid();
            anyhow::ensure!(
                is_trusted_packaged_builder_owner(uid, validation.kernel_overflow_uid),
                "Packaged builder bundle path must be owned by root or the kernel overflow UID: {} (uid: {}, kernel overflow UID: {})",
                path.display(),
                uid,
                validation
                    .kernel_overflow_uid
                    .map_or_else(|| "unreadable".to_string(), |overflow_uid| overflow_uid.to_string())
            );
        }
    }

    Ok(())
}

#[cfg(unix)]
fn is_trusted_packaged_builder_owner(uid: u32, kernel_overflow_uid: Option<u32>) -> bool {
    uid == 0 || kernel_overflow_uid.is_some_and(|overflow_uid| uid == overflow_uid)
}

#[cfg(target_os = "linux")]
fn kernel_overflow_uid() -> Option<u32> {
    fs::read_to_string("/proc/sys/kernel/overflowuid")
        .ok()
        .and_then(|value| value.trim().parse().ok())
}

#[cfg(all(unix, not(target_os = "linux")))]
fn kernel_overflow_uid() -> Option<u32> {
    None
}

fn copy_entry(
    source: &Path,
    destination: &Path,
    optional: bool,
    validation: BuilderBundleValidation,
) -> Result<()> {
    let metadata = match fs::symlink_metadata(source) {
        Ok(metadata) => metadata,
        Err(error) if optional && error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            anyhow::bail!(
                "Required builder bundle path is missing: {}",
                source.display()
            );
        }
        Err(error) => {
            return Err(error).with_context(|| format!("Failed to stat {}", source.display()));
        }
    };

    validate_builder_bundle_entry(source, &metadata, validation)?;

    if metadata.is_dir() {
        copy_dir_recursive(source, destination, validation)?;
    } else {
        copy_path(source, destination, validation)?;
    }

    Ok(())
}

fn copy_path(source: &Path, destination: &Path, validation: BuilderBundleValidation) -> Result<()> {
    let metadata = fs::symlink_metadata(source)
        .with_context(|| format!("Failed to stat {}", source.display()))?;
    validate_builder_bundle_entry(source, &metadata, validation)?;
    anyhow::ensure!(
        metadata.is_file(),
        "Builder bundle path must be a regular file: {}",
        source.display()
    );

    let parent = destination
        .parent()
        .context("Destination path has no parent directory")?;
    fs::create_dir_all(parent).with_context(|| format!("Failed to create {}", parent.display()))?;
    fs::copy(source, destination).with_context(|| {
        format!(
            "Failed to copy {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    fs::set_permissions(destination, metadata.permissions())
        .with_context(|| format!("Failed to set permissions on {}", destination.display()))?;
    Ok(())
}

fn copy_dir_recursive(
    source: &Path,
    destination: &Path,
    validation: BuilderBundleValidation,
) -> Result<()> {
    fs::create_dir_all(destination)
        .with_context(|| format!("Failed to create {}", destination.display()))?;

    for entry in
        fs::read_dir(source).with_context(|| format!("Failed to read {}", source.display()))?
    {
        let entry = entry?;
        let entry_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        let metadata = fs::symlink_metadata(&entry_path)
            .with_context(|| format!("Failed to stat {}", entry_path.display()))?;

        validate_builder_bundle_entry(&entry_path, &metadata, validation)?;

        if file_type.is_dir() {
            copy_dir_recursive(&entry_path, &destination_path, validation)?;
        } else {
            copy_path(&entry_path, &destination_path, validation)?;
        }
    }

    Ok(())
}

/// Find a native package file inside `dist_dir`.
fn find_package_in(dist_dir: &Path) -> Result<PathBuf> {
    for entry in
        fs::read_dir(dist_dir).with_context(|| format!("Failed to read {}", dist_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if is_native_package_file(&path) {
            return Ok(path);
        }
    }

    anyhow::bail!(
        "No native package (.deb, .rpm, or .pkg.tar.*) found in {}",
        dist_dir.display()
    )
}

fn is_native_package_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    name.ends_with(".deb")
        || name.ends_with(".rpm")
        || PACMAN_PACKAGE_SUFFIXES
            .iter()
            .any(|suffix| name.ends_with(suffix))
}

fn build_command_path() -> OsString {
    match std::env::var_os("HOME") {
        Some(home)
            if !home.is_empty()
                && Path::new(&home).is_absolute()
                && !home.to_string_lossy().contains(':') =>
        {
            let mut path = OsString::from(BUILD_COMMAND_PATH);
            path.push(":");
            path.push(Path::new(&home).join(".local/bin"));
            path
        }
        _ => OsString::from(BUILD_COMMAND_PATH),
    }
}

async fn run_and_log(command: &mut Command, log_path: &Path) -> Result<()> {
    let output = command
        .output()
        .await
        .context("Failed to spawn external command")?;

    let mut combined = Vec::new();
    combined.extend_from_slice(&output.stdout);
    combined.extend_from_slice(&output.stderr);
    fs::write(log_path, &combined)
        .with_context(|| format!("Failed to write {}", log_path.display()))?;

    if !output.status.success() {
        anyhow::bail!(
            "Command failed with status {:?}; see {}",
            output.status.code(),
            log_path.display()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RuntimePaths;
    use anyhow::Result;
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    enum FakePackageOutput {
        Deb,
        Rpm,
        Pacman,
    }

    fn write_fake_build_script(path: &Path, output: FakePackageOutput) -> Result<()> {
        let script_body = match output {
            FakePackageOutput::Deb => {
                r#"#!/bin/bash
set -euo pipefail
mkdir -p "${DIST_DIR_OVERRIDE}"
touch "${DIST_DIR_OVERRIDE}/codex-app_${PACKAGE_VERSION}_amd64.deb"
"#
            }
            FakePackageOutput::Rpm => {
                r#"#!/bin/bash
set -euo pipefail
mkdir -p "${DIST_DIR_OVERRIDE}"
touch "${DIST_DIR_OVERRIDE}/codex-app-${PACKAGE_VERSION}.x86_64.rpm"
"#
            }
            FakePackageOutput::Pacman => {
                r#"#!/bin/bash
set -euo pipefail
VER="${PACKAGE_VERSION%%+*}"
mkdir -p "${DIST_DIR_OVERRIDE}"
touch "${DIST_DIR_OVERRIDE}/codex-app-${VER}-1-x86_64.pkg.tar.zst"
"#
            }
        };

        fs::write(path, script_body)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        }
        Ok(())
    }

    fn environment_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[tokio::test]
    async fn builds_update_with_fake_bundle() -> Result<()> {
        let temp = tempdir()?;
        let bundle_root = temp.path().join("bundle");
        let state_root = temp.path().join("state");
        let cache_root = temp.path().join("cache");
        fs::create_dir_all(bundle_root.join("scripts/lib"))?;
        fs::create_dir_all(bundle_root.join("packaging/linux"))?;
        fs::create_dir_all(bundle_root.join("assets"))?;
        fs::write(bundle_root.join("assets/codex.png"), b"png")?;
        fs::write(
            bundle_root.join("packaging/linux/control"),
            "Package: codex",
        )?;
        fs::write(
            bundle_root.join("packaging/linux/codex-app.spec"),
            "Name: codex",
        )?;
        fs::write(
            bundle_root.join("packaging/linux/codex-app.desktop"),
            "[Desktop Entry]",
        )?;
        fs::write(
            bundle_root.join("packaging/linux/codex-app-updater.service"),
            "[Unit]\nDescription=Codex App Updater\n",
        )?;
        fs::write(
            bundle_root.join("install.sh"),
            r#"#!/bin/bash
set -euo pipefail
mkdir -p "${CODEX_INSTALL_DIR}"
echo launcher > "${CODEX_INSTALL_DIR}/start.sh"
chmod +x "${CODEX_INSTALL_DIR}/start.sh"
cat > "${CODEX_INSTALL_DIR}/codex-app-version.env" <<'EOF'
CODEX_APP_UPSTREAM_VERSION=26.422.30944
CODEX_APP_UPSTREAM_BUILD=2080
CODEX_APP_PACKAGE_VERSION=26.422.30944.2080
EOF
"#,
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                bundle_root.join("install.sh"),
                fs::Permissions::from_mode(0o755),
            )?;
        }

        write_fake_build_script(
            &bundle_root.join("scripts/build-deb.sh"),
            FakePackageOutput::Deb,
        )?;
        write_fake_build_script(
            &bundle_root.join("scripts/build-rpm.sh"),
            FakePackageOutput::Rpm,
        )?;
        write_fake_build_script(
            &bundle_root.join("scripts/build-pacman.sh"),
            FakePackageOutput::Pacman,
        )?;
        fs::write(
            bundle_root.join("scripts/patch-linux-window-ui.js"),
            b"console.log('patched');\n",
        )?;
        fs::write(
            bundle_root.join("scripts/lib/package-common.sh"),
            b"#!/bin/bash\n",
        )?;

        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: state_root.join("state.json"),
            log_file: state_root.join("service.log"),
            cache_dir: cache_root.clone(),
            state_dir: state_root.clone(),
            config_dir: temp.path().join("config"),
        };
        paths.ensure_dirs()?;

        let config = RuntimeConfig {
            dmg_url: "https://example.com/Codex.dmg".to_string(),
            initial_check_delay_seconds: 30,
            check_interval_hours: 6,
            auto_install_on_app_exit: true,
            notifications: true,
            developer_mode: false,
            workspace_root: cache_root,
            builder_bundle_root: bundle_root,
            app_executable_path: PathBuf::from("/opt/codex-app/electron"),
            cli_path: None,
        };
        let dmg_path = temp.path().join("Codex.dmg");
        fs::write(&dmg_path, b"dmg")?;

        let mut state = PersistedState::new(true);
        let artifacts = build_update(&config, &mut state, &paths, "678cd508ffe0", &dmg_path)
            .await?
            .expect("new package version should produce build artifacts");
        assert_eq!(state.status, UpdateStatus::ReadyToInstall);
        assert_eq!(
            state.candidate_version.as_deref(),
            Some("26.422.30944.2080")
        );
        assert!(artifacts.workspace_dir.exists());
        assert!(artifacts.package_path.exists());
        assert!(
            artifacts
                .package_path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains("26.422.30944.2080")),
            "expected upstream app version in package filename, got {}",
            artifacts.package_path.display()
        );
        assert!(
            is_native_package_file(&artifacts.package_path),
            "expected a native package (.deb, .rpm, or .pkg.tar.zst), got {}",
            artifacts.package_path.display()
        );
        Ok(())
    }

    #[tokio::test]
    async fn skips_package_rebuild_when_installed_version_already_satisfies_candidate() -> Result<()>
    {
        let temp = tempdir()?;
        let bundle_root = temp.path().join("bundle");
        let state_root = temp.path().join("state");
        let cache_root = temp.path().join("cache");
        fs::create_dir_all(bundle_root.join("scripts/lib"))?;
        fs::create_dir_all(bundle_root.join("packaging/linux"))?;
        fs::create_dir_all(bundle_root.join("assets"))?;
        fs::write(bundle_root.join("assets/codex.png"), b"png")?;
        fs::write(
            bundle_root.join("packaging/linux/control"),
            "Package: codex",
        )?;
        fs::write(
            bundle_root.join("packaging/linux/codex-app.spec"),
            "Name: codex",
        )?;
        fs::write(
            bundle_root.join("packaging/linux/codex-app.desktop"),
            "[Desktop Entry]",
        )?;
        fs::write(
            bundle_root.join("packaging/linux/codex-app-updater.service"),
            "[Unit]\nDescription=Codex App Updater\n",
        )?;
        fs::write(
            bundle_root.join("install.sh"),
            r#"#!/bin/bash
set -euo pipefail
mkdir -p "${CODEX_INSTALL_DIR}"
echo launcher > "${CODEX_INSTALL_DIR}/start.sh"
chmod +x "${CODEX_INSTALL_DIR}/start.sh"
cat > "${CODEX_INSTALL_DIR}/codex-app-version.env" <<'EOF'
CODEX_APP_UPSTREAM_VERSION=26.422.30944
CODEX_APP_UPSTREAM_BUILD=2080
CODEX_APP_PACKAGE_VERSION=26.422.30944.2080
EOF
"#,
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                bundle_root.join("install.sh"),
                fs::Permissions::from_mode(0o755),
            )?;
        }

        let failing_build_script = r#"#!/bin/bash
echo "build script should not run for an already-installed version" >&2
exit 88
"#;
        for script in [
            "scripts/build-deb.sh",
            "scripts/build-rpm.sh",
            "scripts/build-pacman.sh",
        ] {
            let path = bundle_root.join(script);
            fs::write(&path, failing_build_script)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
            }
        }
        fs::write(
            bundle_root.join("scripts/patch-linux-window-ui.js"),
            b"console.log('patched');\n",
        )?;
        fs::write(
            bundle_root.join("scripts/lib/package-common.sh"),
            b"#!/bin/bash\n",
        )?;

        let paths = RuntimePaths {
            config_file: temp.path().join("config/config.toml"),
            state_file: state_root.join("state.json"),
            log_file: state_root.join("service.log"),
            cache_dir: cache_root.clone(),
            state_dir: state_root.clone(),
            config_dir: temp.path().join("config"),
        };
        paths.ensure_dirs()?;

        let config = RuntimeConfig {
            dmg_url: "https://example.com/Codex.dmg".to_string(),
            initial_check_delay_seconds: 30,
            check_interval_hours: 6,
            auto_install_on_app_exit: true,
            notifications: true,
            developer_mode: false,
            workspace_root: cache_root,
            builder_bundle_root: bundle_root,
            app_executable_path: PathBuf::from("/opt/codex-app/electron"),
            cli_path: None,
        };
        let dmg_path = temp.path().join("Codex.dmg");
        fs::write(&dmg_path, b"dmg")?;

        let mut state = PersistedState::new(true);
        state.installed_version = "26.422.30944.2080-1".to_string();
        let artifacts =
            build_update(&config, &mut state, &paths, "678cd508ffe0", &dmg_path).await?;

        assert_eq!(artifacts, None);
        assert_eq!(state.status, UpdateStatus::Idle);
        assert_eq!(state.candidate_version, None);
        assert_eq!(state.artifact_paths.package_path, None);
        Ok(())
    }

    #[test]
    fn workspace_path_component_shortens_full_sha256() -> Result<()> {
        assert_eq!(
            workspace_path_component(
                "678cd508ffe0bdf1f462bcf4e5c8a1559131d6ff4e7f0627856b8d9416198e8f"
            )?,
            "678cd508ffe0bdf1"
        );
        Ok(())
    }

    #[test]
    fn workspace_path_component_rejects_path_characters() {
        assert!(workspace_path_component("../678cd508ffe0").is_err());
    }

    #[test]
    fn build_command_path_ignores_user_environment() {
        let _guard = environment_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let original_path = std::env::var_os("PATH");
        let original_home = std::env::var_os("HOME");
        std::env::set_var("PATH", "/tmp/malicious:/home/user/bin");
        std::env::set_var("HOME", "/home/user");
        assert_eq!(
            build_command_path(),
            OsString::from("/usr/local/sbin:/usr/local/bin:/usr/bin:/bin:/home/user/.local/bin")
        );
        match original_path {
            Some(path) => std::env::set_var("PATH", path),
            None => std::env::remove_var("PATH"),
        }
        match original_home {
            Some(path) => std::env::set_var("HOME", path),
            None => std::env::remove_var("HOME"),
        }
    }

    #[test]
    fn build_command_path_rejects_home_path_injection() {
        let _guard = environment_lock()
            .lock()
            .expect("environment lock should not be poisoned");
        let original_path = std::env::var_os("PATH");
        let original_home = std::env::var_os("HOME");
        std::env::set_var("PATH", "/tmp/malicious");
        std::env::set_var("HOME", "/home/user:/tmp/malicious");
        assert_eq!(build_command_path(), OsString::from(BUILD_COMMAND_PATH));
        std::env::set_var("HOME", "relative-home");
        assert_eq!(build_command_path(), OsString::from(BUILD_COMMAND_PATH));
        match original_path {
            Some(path) => std::env::set_var("PATH", path),
            None => std::env::remove_var("PATH"),
        }
        match original_home {
            Some(path) => std::env::set_var("HOME", path),
            None => std::env::remove_var("HOME"),
        }
    }

    #[test]
    fn bundle_copy_skips_missing_optional_package_scripts() -> Result<()> {
        let temp = tempdir()?;
        let source_root = temp.path().join("source");
        let destination_root = temp.path().join("destination");

        fs::create_dir_all(source_root.join("scripts/lib"))?;
        fs::create_dir_all(source_root.join("packaging/linux"))?;
        fs::create_dir_all(source_root.join("assets"))?;
        fs::write(source_root.join("install.sh"), b"#!/bin/bash\n")?;
        fs::write(source_root.join("scripts/build-deb.sh"), b"#!/bin/bash\n")?;
        fs::write(
            source_root.join("scripts/patch-linux-window-ui.js"),
            b"console.log('patched');\n",
        )?;
        fs::write(
            source_root.join("scripts/lib/package-common.sh"),
            b"#!/bin/bash\n",
        )?;
        fs::write(
            source_root.join("packaging/linux/control"),
            b"Package: codex\n",
        )?;
        fs::write(
            source_root.join("packaging/linux/codex-app-updater.service"),
            b"[Unit]\nDescription=Codex App Updater\n",
        )?;
        fs::write(source_root.join("assets/codex.png"), b"png")?;

        copy_builder_bundle(&source_root, &destination_root, false)?;

        assert!(destination_root.join("scripts/build-deb.sh").exists());
        assert!(destination_root
            .join("scripts/patch-linux-window-ui.js")
            .exists());
        assert!(!destination_root.join("scripts/build-rpm.sh").exists());
        assert!(!destination_root.join("scripts/build-pacman.sh").exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn bundle_copy_rejects_symlinked_builder_entries() -> Result<()> {
        use std::os::unix::fs::symlink;

        let temp = tempdir()?;
        let source_root = temp.path().join("source");
        let destination_root = temp.path().join("destination");
        let external_script = temp.path().join("external-install.sh");

        fs::create_dir_all(&source_root)?;
        fs::write(&external_script, b"#!/bin/bash\n")?;
        symlink(&external_script, source_root.join("install.sh"))?;

        let error = copy_builder_bundle(&source_root, &destination_root, false)
            .expect_err("builder symlink should be rejected");

        assert!(error.to_string().contains("must not be a symlink"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn bundle_copy_rejects_writable_production_root() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempdir()?;
        let source_root = temp.path().join("source");
        let destination_root = temp.path().join("destination");
        fs::create_dir_all(&source_root)?;
        fs::set_permissions(&source_root, fs::Permissions::from_mode(0o777))?;

        let error = copy_builder_bundle(&source_root, &destination_root, false)
            .expect_err("production builder root should not be group/world writable");

        assert!(error
            .to_string()
            .contains("must not be group- or world-writable"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn bundle_copy_rejects_writable_production_entry() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempdir()?;
        let source_root = temp.path().join("source");
        let destination_root = temp.path().join("destination");

        fs::create_dir_all(source_root.join("scripts/lib"))?;
        fs::create_dir_all(source_root.join("packaging/linux"))?;
        fs::create_dir_all(source_root.join("assets"))?;
        fs::write(source_root.join("install.sh"), b"#!/bin/bash\n")?;
        fs::write(source_root.join("scripts/build-deb.sh"), b"#!/bin/bash\n")?;
        fs::write(
            source_root.join("scripts/patch-linux-window-ui.js"),
            b"console.log('patched');\n",
        )?;
        fs::write(
            source_root.join("scripts/lib/package-common.sh"),
            b"#!/bin/bash\n",
        )?;
        fs::write(
            source_root.join("packaging/linux/control"),
            b"Package: codex\n",
        )?;
        fs::write(
            source_root.join("packaging/linux/codex-app-updater.service"),
            b"[Unit]\nDescription=Codex App Updater\n",
        )?;
        fs::write(source_root.join("assets/codex.png"), b"png")?;
        fs::set_permissions(
            source_root.join("packaging/linux/control"),
            fs::Permissions::from_mode(0o666),
        )?;

        let error = copy_builder_bundle(&source_root, &destination_root, false)
            .expect_err("production builder entries should not be group/world writable");

        assert!(error
            .to_string()
            .contains("must not be group- or world-writable"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn packaged_builder_owner_allows_root_and_kernel_overflow_uid() {
        let overflow_uid = kernel_overflow_uid();
        assert!(is_trusted_packaged_builder_owner(0, overflow_uid));

        if let Some(overflow_uid) = overflow_uid {
            assert!(is_trusted_packaged_builder_owner(
                overflow_uid,
                Some(overflow_uid)
            ));
        }

        let untrusted_uid = (1..=u32::MAX)
            .find(|uid| *uid != 0 && Some(*uid) != overflow_uid)
            .expect("there should be an untrusted uid value");
        assert!(!is_trusted_packaged_builder_owner(
            untrusted_uid,
            overflow_uid
        ));
    }

    #[cfg(unix)]
    #[test]
    fn bundle_copy_allows_writable_developer_root_to_reach_required_file_validation() -> Result<()>
    {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempdir()?;
        let source_root = temp.path().join("source");
        let destination_root = temp.path().join("destination");
        fs::create_dir_all(&source_root)?;
        fs::set_permissions(&source_root, fs::Permissions::from_mode(0o777))?;

        let error = copy_builder_bundle(&source_root, &destination_root, true)
            .expect_err("developer mode should continue to required-file validation");

        assert!(error
            .to_string()
            .contains("Required builder bundle path is missing"));
        Ok(())
    }

    #[test]
    fn returns_error_when_dist_has_no_native_package() -> Result<()> {
        let temp = tempdir()?;
        fs::write(temp.path().join("README.txt"), b"no packages here")?;

        let error = find_package_in(temp.path()).expect_err("package discovery should fail");
        assert!(error
            .to_string()
            .contains("No native package (.deb, .rpm, or .pkg.tar.*)"));
        Ok(())
    }

    #[test]
    fn finds_pacman_package_in_dist_dir() -> Result<()> {
        let temp = tempdir()?;
        let pkg_path = temp
            .path()
            .join("codex-app-2026.03.30.120000-1-x86_64.pkg.tar.zst");
        fs::write(&pkg_path, b"pkg")?;

        let found = find_package_in(temp.path())?;
        assert_eq!(found, pkg_path);
        Ok(())
    }
}
