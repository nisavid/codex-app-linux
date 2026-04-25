//! Installation helpers for privileged and non-privileged package application.

use anyhow::{Context, Result};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    path::{Path, PathBuf},
    process,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

const PACKAGE_NAME: &str = "codex-app";
const LEGACY_PACKAGE_NAME: &str = "codex-desktop";
const INSTALLED_UPDATER_BINARY: &str = "/usr/bin/codex-app-updater";
const APT_CANDIDATES: &[&str] = &["/usr/bin/apt", "/bin/apt"];
const DNF_CANDIDATES: &[&str] = &["/usr/bin/dnf", "/bin/dnf", "/usr/bin/dnf5", "/bin/dnf5"];
const DPKG_CANDIDATES: &[&str] = &["/usr/bin/dpkg", "/bin/dpkg"];
const DPKG_DEB_CANDIDATES: &[&str] = &["/usr/bin/dpkg-deb", "/bin/dpkg-deb"];
const DPKG_QUERY_CANDIDATES: &[&str] = &["/usr/bin/dpkg-query", "/bin/dpkg-query"];
const RPM_CANDIDATES: &[&str] = &["/usr/bin/rpm", "/bin/rpm"];
const PACMAN_CANDIDATES: &[&str] = &["/usr/bin/pacman", "/bin/pacman"];
const VERCMP_CANDIDATES: &[&str] = &["/usr/bin/vercmp", "/bin/vercmp"];
const PACMAN_PACKAGE_SUFFIXES: &[&str] = &[
    ".pkg.tar.zst",
    ".pkg.tar.xz",
    ".pkg.tar.gz",
    ".pkg.tar.bz2",
    ".pkg.tar.lz",
    ".pkg.tar.lz4",
    ".pkg.tar.lz5",
];

/// The native package format in use on the current system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    Deb,
    Rpm,
    Pacman,
}

impl PackageKind {
    pub fn detect() -> Self {
        detect_package_kind(
            program_exists(PACMAN_CANDIDATES, "pacman"),
            program_exists(DPKG_CANDIDATES, "dpkg"),
            program_exists(RPM_CANDIDATES, "rpm"),
            installed_pacman_version() != "unknown",
            installed_deb_version() != "unknown",
            installed_rpm_version() != "unknown",
            os_release_fields(),
        )
    }

    pub fn from_path(path: &Path) -> Self {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if is_pacman_package_file_name(file_name) {
            return Self::Pacman;
        }

        match path.extension().and_then(|e| e.to_str()) {
            Some("rpm") => Self::Rpm,
            _ => Self::Deb,
        }
    }
}

fn detect_package_kind(
    has_pacman: bool,
    has_dpkg: bool,
    has_rpm: bool,
    pacman_installed: bool,
    deb_installed: bool,
    rpm_installed: bool,
    os_release: Option<(String, String)>,
) -> PackageKind {
    if pacman_installed {
        return PackageKind::Pacman;
    }
    if deb_installed {
        return PackageKind::Deb;
    }
    if rpm_installed {
        return PackageKind::Rpm;
    }

    if let Some((id, id_like)) = os_release {
        let fields = [id.as_str(), id_like.as_str()];
        if os_release_matches(
            &fields,
            &["arch", "archlinux", "manjaro", "endeavouros", "artix"],
        ) {
            return PackageKind::Pacman;
        }
        if os_release_matches(
            &fields,
            &[
                "debian",
                "ubuntu",
                "linuxmint",
                "pop",
                "elementary",
                "zorin",
            ],
        ) {
            return PackageKind::Deb;
        }
        if os_release_matches(
            &fields,
            &[
                "fedora",
                "rhel",
                "centos",
                "rocky",
                "almalinux",
                "ol",
                "sles",
                "suse",
                "opensuse",
            ],
        ) {
            return PackageKind::Rpm;
        }
    }

    if has_dpkg {
        PackageKind::Deb
    } else if has_rpm {
        PackageKind::Rpm
    } else if has_pacman {
        PackageKind::Pacman
    } else {
        PackageKind::Deb
    }
}

fn os_release_fields() -> Option<(String, String)> {
    let contents = fs::read_to_string("/etc/os-release").ok()?;
    let mut id = String::new();
    let mut id_like = String::new();

    for line in contents.lines() {
        if let Some(value) = line.strip_prefix("ID=") {
            id = trim_os_release_value(value).to_ascii_lowercase();
        } else if let Some(value) = line.strip_prefix("ID_LIKE=") {
            id_like = trim_os_release_value(value).to_ascii_lowercase();
        }
    }

    Some((id, id_like))
}

fn trim_os_release_value(value: &str) -> &str {
    value.trim().trim_matches('"').trim_matches('\'')
}

fn os_release_matches(fields: &[&str], expected: &[&str]) -> bool {
    fields.iter().any(|field| {
        field
            .split_whitespace()
            .any(|token| expected.iter().any(|candidate| token == *candidate))
    })
}

/// Returns the currently installed package version when available.
pub fn installed_package_version() -> String {
    match PackageKind::detect() {
        PackageKind::Deb => installed_deb_version(),
        PackageKind::Rpm => installed_rpm_version(),
        PackageKind::Pacman => installed_pacman_version(),
    }
}

/// Returns whether the primary native package still appears to be installed.
pub fn is_primary_package_installed() -> bool {
    installed_package_version() != "unknown"
}

fn installed_deb_version() -> String {
    let version = installed_version_from_command(
        &program_path(DPKG_QUERY_CANDIDATES, "dpkg-query"),
        &["-W", "-f=${Version}", PACKAGE_NAME],
    );
    if version != "unknown" {
        return version;
    }
    installed_version_from_command(
        &program_path(DPKG_QUERY_CANDIDATES, "dpkg-query"),
        &["-W", "-f=${Version}", LEGACY_PACKAGE_NAME],
    )
}

fn installed_rpm_version() -> String {
    let version = installed_version_from_command(
        &program_path(RPM_CANDIDATES, "rpm"),
        &["-q", "--queryformat", "%{VERSION}-%{RELEASE}", PACKAGE_NAME],
    );
    if version != "unknown" {
        return version;
    }
    installed_version_from_command(
        &program_path(RPM_CANDIDATES, "rpm"),
        &[
            "-q",
            "--queryformat",
            "%{VERSION}-%{RELEASE}",
            LEGACY_PACKAGE_NAME,
        ],
    )
}

fn installed_pacman_version() -> String {
    for package_name in [PACKAGE_NAME, LEGACY_PACKAGE_NAME] {
        match Command::new(program_path(PACMAN_CANDIDATES, "pacman"))
            .args(["-Q", package_name])
            .output()
        {
            Ok(output) if output.status.success() => {
                return parse_pacman_installed_version(output.stdout);
            }
            _ => {}
        }
    }
    "unknown".to_string()
}

/// Installs a rebuilt Debian package on the local machine.
pub fn install_deb(path: &Path) -> Result<()> {
    let staged = stage_install_candidate(path, PackageKind::Deb)?;
    let metadata = deb_package_metadata(&staged.path)?;
    ensure_deb_package_identity(&metadata, &staged.path)?;
    ensure_upgrade_path(&metadata.version)?;

    if program_exists(APT_CANDIDATES, "apt") {
        let mut command = apt_install_command(&staged.path)?;
        run_install(&mut command).context("apt install failed")?;
        return Ok(());
    }

    let mut command = dpkg_install_command(&staged.path);
    run_install(&mut command).context("dpkg -i failed")
}

/// Installs a rebuilt RPM package on the local machine.
pub fn install_rpm(path: &Path) -> Result<()> {
    let staged = stage_install_candidate(path, PackageKind::Rpm)?;
    let metadata = rpm_package_metadata(&staged.path)?;
    ensure_rpm_package_identity(&metadata, &staged.path)?;

    if program_exists(DNF_CANDIDATES, "dnf") || program_exists(DNF_CANDIDATES, "dnf5") {
        let mut command = dnf_install_command(&staged.path)?;
        run_install(&mut command).context("dnf install failed")?;
        return Ok(());
    }

    let mut command = rpm_install_command(&staged.path);
    run_install(&mut command).context("rpm -Uvh failed")
}

/// Installs a rebuilt pacman package on the local machine.
pub fn install_pacman(path: &Path) -> Result<()> {
    let staged = stage_install_candidate(path, PackageKind::Pacman)?;
    let metadata = pacman_package_metadata(&staged.path)?;
    ensure_pacman_package_identity(&metadata, &staged.path)?;
    ensure_upgrade_path_pacman(&metadata.version)?;

    let mut command = pacman_install_command(&staged.path);
    run_install(&mut command).context("pacman -U failed")
}

/// Builds the `pkexec` command used for privileged package installation.
pub fn pkexec_command(current_exe: &Path, package_path: &Path) -> Command {
    let updater_binary = updater_binary_for_privileged_install(current_exe);
    let subcommand = match PackageKind::from_path(package_path) {
        PackageKind::Rpm => "install-rpm",
        PackageKind::Deb => "install-deb",
        PackageKind::Pacman => "install-pacman",
    };
    let mut command = Command::new("pkexec");
    command
        .arg(updater_binary)
        .arg(subcommand)
        .arg("--path")
        .arg(package_path);
    command
}

fn run_install(command: &mut Command) -> Result<()> {
    let status = command
        .status()
        .context("Failed to execute installation command")?;
    anyhow::ensure!(
        status.success(),
        "installation command exited with {status}"
    );
    Ok(())
}

#[derive(Debug)]
struct StagedPackage {
    path: PathBuf,
    dir: PathBuf,
}

impl Drop for StagedPackage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn stage_install_candidate(path: &Path, expected_kind: PackageKind) -> Result<StagedPackage> {
    ensure_install_candidate_source(path, expected_kind)?;

    let file_name = path
        .file_name()
        .with_context(|| format!("Package path has no file name: {}", path.display()))?;
    let dir = private_install_stage_dir();
    fs::create_dir(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    #[cfg(unix)]
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("Failed to restrict {}", dir.display()))?;

    let staged_path = dir.join(file_name);
    fs::copy(path, &staged_path).with_context(|| {
        format!(
            "Failed to copy install candidate {} to {}",
            path.display(),
            staged_path.display()
        )
    })?;

    Ok(StagedPackage {
        path: staged_path,
        dir,
    })
}

fn ensure_install_candidate_source(path: &Path, expected_kind: PackageKind) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("Package not found: {}", path.display()))?;
    anyhow::ensure!(
        !metadata.file_type().is_symlink(),
        "Refusing to install package through symlink: {}",
        path.display()
    );
    anyhow::ensure!(
        metadata.file_type().is_file(),
        "Package path is not a regular file: {}",
        path.display()
    );

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| format!("Package path has no UTF-8 file name: {}", path.display()))?;

    match expected_kind {
        PackageKind::Deb => {
            anyhow::ensure!(
                file_name.starts_with("codex-app_") && file_name.ends_with(".deb"),
                "Debian package filename must match codex-app_*.deb: {file_name}"
            );
        }
        PackageKind::Rpm => {
            anyhow::ensure!(
                file_name.starts_with("codex-app-") && file_name.ends_with(".rpm"),
                "RPM package filename must match codex-app-*.rpm: {file_name}"
            );
        }
        PackageKind::Pacman => {
            anyhow::ensure!(
                file_name.starts_with("codex-app-") && is_pacman_package_file_name(file_name),
                "Pacman package filename must match codex-app-*.pkg.tar.*: {file_name}"
            );
        }
    }

    Ok(())
}

fn private_install_stage_dir() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "codex-app-updater-install.{}.{}",
        process::id(),
        timestamp
    ))
}

fn installed_version_from_command(program: &Path, args: &[&str]) -> String {
    match Command::new(program).args(args).output() {
        Ok(output) if output.status.success() => parse_installed_version(output.stdout),
        _ => "unknown".to_string(),
    }
}

fn parse_installed_version(stdout: Vec<u8>) -> String {
    let version = String::from_utf8_lossy(&stdout).trim().to_string();
    if version.is_empty() {
        "unknown".to_string()
    } else {
        version
    }
}

fn parse_pacman_installed_version(stdout: Vec<u8>) -> String {
    let text = String::from_utf8_lossy(&stdout);
    let version = text
        .split_whitespace()
        .nth(1)
        .unwrap_or("")
        .trim()
        .to_string();
    if version.is_empty() {
        "unknown".to_string()
    } else {
        version
    }
}

fn ensure_upgrade_path(candidate: &str) -> Result<()> {
    let installed = installed_package_version();
    if installed == "unknown" {
        return Ok(());
    }

    anyhow::ensure!(
        is_version_newer(candidate, &installed)?,
        "Refusing to install non-newer package version {candidate} over installed version {installed}"
    );
    Ok(())
}

fn ensure_upgrade_path_pacman(candidate: &str) -> Result<()> {
    let installed = installed_pacman_version();
    if installed == "unknown" {
        return Ok(());
    }

    anyhow::ensure!(
        is_version_newer_pacman(candidate, &installed)?,
        "Refusing to install non-newer package version {candidate} over installed version {installed}"
    );
    Ok(())
}

fn apt_install_command(path: &Path) -> Result<Command> {
    install_command_in_parent(&program_path(APT_CANDIDATES, "apt"), path)
}

fn dpkg_install_command(path: &Path) -> Command {
    let mut command = Command::new(program_path(DPKG_CANDIDATES, "dpkg"));
    command.arg("-i").arg(path.as_os_str());
    command
}

fn dnf_install_command(path: &Path) -> Result<Command> {
    install_command_in_parent(&program_path(DNF_CANDIDATES, "dnf"), path)
}

fn install_command_in_parent(program: &Path, path: &Path) -> Result<Command> {
    let program_name = program
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("package manager");
    let parent = path
        .parent()
        .with_context(|| format!("{program_name} package path has no parent directory"))?;
    let file_name = path
        .file_name()
        .with_context(|| format!("{program_name} package path has no file name"))?
        .to_string_lossy()
        .into_owned();

    let mut command = Command::new(program);
    command
        .current_dir(parent)
        .arg("install")
        .arg("-y")
        .arg(format!("./{file_name}"));
    Ok(command)
}

fn rpm_install_command(path: &Path) -> Command {
    let mut command = Command::new(program_path(RPM_CANDIDATES, "rpm"));
    command.args(["-Uvh"]).arg(path.as_os_str());
    command
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RpmPackageMetadata {
    name: String,
    version_release: String,
    arch: String,
}

fn rpm_package_metadata(path: &Path) -> Result<RpmPackageMetadata> {
    let output = Command::new(program_path(RPM_CANDIDATES, "rpm"))
        .args([
            "-qp",
            "--queryformat",
            "%{NAME}\n%{VERSION}-%{RELEASE}\n%{ARCH}",
        ])
        .arg(path)
        .output()
        .context("Failed to inspect RPM package metadata")?;

    anyhow::ensure!(
        output.status.success(),
        "rpm could not read package metadata from {}",
        path.display()
    );

    parse_rpm_package_metadata(&output.stdout, path)
}

fn parse_rpm_package_metadata(output: &[u8], path: &Path) -> Result<RpmPackageMetadata> {
    let text = String::from_utf8(output.to_vec()).context("rpm returned non-UTF8 metadata")?;
    let mut lines = text.lines().map(str::trim);
    let name = lines.next().unwrap_or("").to_string();
    let version_release = lines.next().unwrap_or("").to_string();
    let arch = lines.next().unwrap_or("").to_string();

    anyhow::ensure!(
        !name.is_empty(),
        "rpm returned an empty package name for {}",
        path.display()
    );
    anyhow::ensure!(
        !version_release.is_empty() && version_release != "-",
        "rpm returned an empty package version for {}",
        path.display()
    );
    anyhow::ensure!(
        !arch.is_empty(),
        "rpm returned an empty package architecture for {}",
        path.display()
    );

    Ok(RpmPackageMetadata {
        name,
        version_release,
        arch,
    })
}

fn ensure_rpm_package_identity(metadata: &RpmPackageMetadata, path: &Path) -> Result<()> {
    anyhow::ensure!(
        metadata.name == PACKAGE_NAME,
        "RPM package {} has unexpected name {}; expected {PACKAGE_NAME}",
        path.display(),
        metadata.name
    );
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DebPackageMetadata {
    name: String,
    version: String,
    arch: String,
}

fn deb_package_metadata(path: &Path) -> Result<DebPackageMetadata> {
    let name = deb_package_field(path, "Package")?;
    let version = deb_package_field(path, "Version")?;
    let arch = deb_package_field(path, "Architecture")?;

    Ok(DebPackageMetadata {
        name,
        version,
        arch,
    })
}

fn deb_package_field(path: &Path, field: &str) -> Result<String> {
    let output = Command::new(program_path(DPKG_DEB_CANDIDATES, "dpkg-deb"))
        .arg("-f")
        .arg(path)
        .arg(field)
        .output()
        .with_context(|| format!("Failed to inspect Debian package {field} metadata"))?;

    anyhow::ensure!(
        output.status.success(),
        "dpkg-deb could not read package {field} metadata from {}",
        path.display()
    );

    let value = String::from_utf8(output.stdout)
        .with_context(|| format!("dpkg-deb returned non-UTF8 package {field} metadata"))?
        .trim()
        .to_string();
    anyhow::ensure!(
        !value.is_empty(),
        "dpkg-deb returned empty package {field} metadata for {}",
        path.display()
    );
    Ok(value)
}

fn ensure_deb_package_identity(metadata: &DebPackageMetadata, path: &Path) -> Result<()> {
    anyhow::ensure!(
        metadata.name == PACKAGE_NAME,
        "Debian package {} has unexpected name {}; expected {PACKAGE_NAME}",
        path.display(),
        metadata.name
    );
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PacmanPackageMetadata {
    name: String,
    version: String,
}

fn pacman_package_metadata(path: &Path) -> Result<PacmanPackageMetadata> {
    let output = Command::new(program_path(PACMAN_CANDIDATES, "pacman"))
        .args(["-Qp"])
        .arg(path)
        .output()
        .context("Failed to inspect pacman package metadata")?;

    anyhow::ensure!(
        output.status.success(),
        "pacman could not read package metadata from {}",
        path.display()
    );

    parse_pacman_package_metadata(&output.stdout, path)
}

fn parse_pacman_package_metadata(output: &[u8], path: &Path) -> Result<PacmanPackageMetadata> {
    let text = String::from_utf8(output.to_vec()).context("pacman returned non-UTF8 metadata")?;
    let mut parts = text.split_whitespace();
    let name = parts.next().unwrap_or("").to_string();
    let version = parts.next().unwrap_or("").to_string();

    anyhow::ensure!(
        !name.is_empty(),
        "pacman returned an empty package name for {}",
        path.display()
    );
    anyhow::ensure!(
        !version.is_empty(),
        "pacman returned an empty package version for {}",
        path.display()
    );

    Ok(PacmanPackageMetadata { name, version })
}

fn ensure_pacman_package_identity(metadata: &PacmanPackageMetadata, path: &Path) -> Result<()> {
    anyhow::ensure!(
        metadata.name == PACKAGE_NAME,
        "Pacman package {} has unexpected name {}; expected {PACKAGE_NAME}",
        path.display(),
        metadata.name
    );
    Ok(())
}

fn pacman_install_command(path: &Path) -> Command {
    let mut command = Command::new(program_path(PACMAN_CANDIDATES, "pacman"));
    command.args(["-U", "--noconfirm"]).arg(path.as_os_str());
    command
}

fn updater_binary_for_privileged_install(current_exe: &Path) -> PathBuf {
    let installed = PathBuf::from(INSTALLED_UPDATER_BINARY);
    if installed.is_file() {
        installed
    } else {
        current_exe.to_path_buf()
    }
}

fn is_version_newer(candidate: &str, installed: &str) -> Result<bool> {
    let status = Command::new(program_path(DPKG_CANDIDATES, "dpkg"))
        .args(["--compare-versions", candidate, "gt", installed])
        .status()
        .context("Failed to compare Debian package versions")?;
    Ok(status.success())
}

fn pacman_package_version(path: &Path) -> Result<String> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .context("Package path has no file name")?;

    let stripped = strip_pacman_package_suffix(file_name)
        .with_context(|| format!("Not a valid pacman package filename: {file_name}"))?;
    let prefix = format!("{PACKAGE_NAME}-");
    let without_name = stripped
        .strip_prefix(&prefix)
        .with_context(|| format!("Pacman package filename does not start with {prefix}"))?;
    let (version_release, _arch) = without_name
        .rsplit_once('-')
        .context("Pacman package filename is missing an architecture suffix")?;
    anyhow::ensure!(
        !version_release.is_empty(),
        "Could not parse package version from {file_name}"
    );
    Ok(version_release.to_string())
}

fn is_version_newer_pacman(candidate: &str, installed: &str) -> Result<bool> {
    let output = Command::new(program_path(VERCMP_CANDIDATES, "vercmp"))
        .args([candidate, installed])
        .output()
        .context("Failed to compare pacman package versions")?;
    anyhow::ensure!(
        output.status.success(),
        "vercmp exited with status {}",
        output.status
    );

    let comparison = String::from_utf8(output.stdout)
        .context("vercmp returned a non-UTF8 response")?
        .trim()
        .parse::<i32>()
        .context("vercmp returned an invalid comparison value")?;
    Ok(comparison > 0)
}

fn strip_pacman_package_suffix(file_name: &str) -> Option<&str> {
    let lower = file_name.to_ascii_lowercase();
    PACMAN_PACKAGE_SUFFIXES.iter().find_map(|suffix| {
        lower
            .strip_suffix(suffix)
            .map(|_| &file_name[..file_name.len() - suffix.len()])
    })
}

fn is_pacman_package_file_name(file_name: &str) -> bool {
    strip_pacman_package_suffix(file_name).is_some()
}

fn program_exists(candidates: &[&str], fallback: &str) -> bool {
    candidates.iter().any(|path| Path::new(path).is_file()) || command_exists(fallback)
}

fn program_path(candidates: &[&str], fallback: &str) -> PathBuf {
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from(fallback))
}

fn command_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|path| {
            std::env::split_paths(&path).any(|entry| {
                let candidate: PathBuf = entry.join(name);
                candidate.is_file()
            })
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn builds_pkexec_command_for_privileged_deb_install() {
        let command = pkexec_command(
            Path::new("/usr/bin/codex-app-updater"),
            Path::new("/tmp/update.deb"),
        );
        let args: Vec<_> = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec![
                "/usr/bin/codex-app-updater",
                "install-deb",
                "--path",
                "/tmp/update.deb"
            ]
        );
    }

    #[test]
    fn builds_pkexec_command_for_privileged_rpm_install() {
        let command = pkexec_command(
            Path::new("/usr/bin/codex-app-updater"),
            Path::new("/tmp/update.rpm"),
        );
        let args: Vec<_> = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec![
                "/usr/bin/codex-app-updater",
                "install-rpm",
                "--path",
                "/tmp/update.rpm"
            ]
        );
    }

    #[test]
    fn prefers_installed_updater_path_for_pkexec() {
        let selected =
            updater_binary_for_privileged_install(Path::new("/tmp/codex-app-updater-old"));
        let expected = if Path::new("/usr/bin/codex-app-updater").is_file() {
            PathBuf::from("/usr/bin/codex-app-updater")
        } else {
            PathBuf::from("/tmp/codex-app-updater-old")
        };
        assert_eq!(selected, expected);
    }

    #[test]
    fn builds_local_apt_install_command() -> Result<()> {
        let command = apt_install_command(Path::new("/tmp/build/codex.deb"))?;
        assert!(command.get_program().to_string_lossy().ends_with("apt"));
        assert_eq!(
            command
                .get_args()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["install", "-y", "./codex.deb"]
        );
        Ok(())
    }

    #[test]
    fn builds_local_dnf_install_command() -> Result<()> {
        let command = dnf_install_command(Path::new("/tmp/build/codex.rpm"))?;
        let program = command.get_program().to_string_lossy();
        assert!(program.ends_with("dnf") || program.ends_with("dnf5"));
        assert_eq!(
            command
                .get_args()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["install", "-y", "./codex.rpm"]
        );
        Ok(())
    }

    #[test]
    fn package_kind_from_path_detects_rpm() {
        assert_eq!(
            PackageKind::from_path(Path::new("/tmp/codex.rpm")),
            PackageKind::Rpm
        );
    }

    #[test]
    fn package_kind_from_path_detects_deb() {
        assert_eq!(
            PackageKind::from_path(Path::new("/tmp/codex.deb")),
            PackageKind::Deb
        );
    }

    #[test]
    fn package_kind_from_path_detects_pacman_zst() {
        assert_eq!(
            PackageKind::from_path(Path::new("/tmp/codex-app-2026.03.30-1-x86_64.pkg.tar.zst")),
            PackageKind::Pacman
        );
    }

    #[test]
    fn package_kind_from_path_detects_pacman_xz() {
        assert_eq!(
            PackageKind::from_path(Path::new("/tmp/codex-app-2026.03.30-1-x86_64.pkg.tar.xz")),
            PackageKind::Pacman
        );
    }

    #[test]
    fn detection_prefers_installed_pacman_package_even_if_rpm_exists() {
        assert_eq!(
            detect_package_kind(
                true,
                false,
                true,
                true,
                false,
                false,
                Some(("arch".to_string(), "".to_string())),
            ),
            PackageKind::Pacman
        );
    }

    #[test]
    fn detection_uses_arch_os_release_when_nothing_is_installed() {
        assert_eq!(
            detect_package_kind(
                true,
                false,
                true,
                false,
                false,
                false,
                Some(("arch".to_string(), "".to_string())),
            ),
            PackageKind::Pacman
        );
    }

    #[test]
    fn detection_uses_debian_os_release_before_rpm_command_presence() {
        assert_eq!(
            detect_package_kind(
                false,
                true,
                true,
                false,
                false,
                false,
                Some(("ubuntu".to_string(), "debian".to_string())),
            ),
            PackageKind::Deb
        );
    }

    #[test]
    fn detection_uses_rpm_os_release_before_pacman_command_presence() {
        assert_eq!(
            detect_package_kind(
                true,
                false,
                true,
                false,
                false,
                false,
                Some(("fedora".to_string(), "rhel".to_string())),
            ),
            PackageKind::Rpm
        );
    }

    #[test]
    fn trims_quoted_os_release_values() {
        assert_eq!(trim_os_release_value("\"arch\""), "arch");
        assert_eq!(trim_os_release_value("'debian ubuntu'"), "debian ubuntu");
    }

    #[test]
    fn matches_expected_os_release_tokens() {
        assert!(os_release_matches(&["ubuntu debian", ""], &["debian"]));
        assert!(!os_release_matches(&["ubuntu", ""], &["fedora"]));
    }

    #[test]
    fn builds_pkexec_command_for_privileged_pacman_install() {
        let command = pkexec_command(
            Path::new("/usr/bin/codex-app-updater"),
            Path::new("/tmp/update.pkg.tar.zst"),
        );
        let args: Vec<_> = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec![
                "/usr/bin/codex-app-updater",
                "install-pacman",
                "--path",
                "/tmp/update.pkg.tar.zst"
            ]
        );
    }

    #[test]
    fn rejects_symlink_install_candidates() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let target = temp.path().join("codex-app_26.422.30944.2080_amd64.deb");
        let link = temp.path().join("linked.deb");
        std::fs::write(&target, b"package")?;
        std::os::unix::fs::symlink(&target, &link)?;

        let error = stage_install_candidate(&link, PackageKind::Deb)
            .expect_err("symlink package path should be rejected");

        assert!(error.to_string().contains("symlink"));
        Ok(())
    }

    #[test]
    fn rejects_mismatched_install_candidate_kind() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("codex-app-26.422.30944.2080-1.x86_64.rpm");
        std::fs::write(&path, b"package")?;

        let error = stage_install_candidate(&path, PackageKind::Deb)
            .expect_err("RPM path should not be accepted for Debian install");

        assert!(error.to_string().contains("Debian package"));
        Ok(())
    }

    #[test]
    fn parses_rpm_package_metadata() -> Result<()> {
        let metadata = parse_rpm_package_metadata(
            b"codex-app\n26.422.30944.2080-1\nx86_64".as_slice(),
            Path::new("/tmp/codex-app-26.422.30944.2080-1.x86_64.rpm"),
        )?;

        assert_eq!(metadata.name, "codex-app");
        assert_eq!(metadata.version_release, "26.422.30944.2080-1");
        assert_eq!(metadata.arch, "x86_64");
        Ok(())
    }

    #[test]
    fn rejects_rpm_package_metadata_for_wrong_name() -> Result<()> {
        let metadata = RpmPackageMetadata {
            name: "other-app".to_string(),
            version_release: "26.422.30944.2080-1".to_string(),
            arch: "x86_64".to_string(),
        };

        let error = ensure_rpm_package_identity(&metadata, Path::new("/tmp/codex-app.rpm"))
            .expect_err("wrong RPM package name should be rejected");

        assert!(error.to_string().contains("codex-app"));
        Ok(())
    }

    #[test]
    fn rejects_deb_package_metadata_for_wrong_name() {
        let metadata = DebPackageMetadata {
            name: "other-app".to_string(),
            version: "26.422.30944.2080".to_string(),
            arch: "amd64".to_string(),
        };

        let error = ensure_deb_package_identity(&metadata, Path::new("/tmp/codex-app.deb"))
            .expect_err("wrong Debian package name should be rejected");

        assert!(error.to_string().contains("codex-app"));
    }

    #[test]
    fn parses_pacman_package_metadata() -> Result<()> {
        let metadata = parse_pacman_package_metadata(
            b"codex-app 26.422.30944.2080-1\n".as_slice(),
            Path::new("/tmp/codex-app-26.422.30944.2080-1-x86_64.pkg.tar.zst"),
        )?;

        assert_eq!(metadata.name, "codex-app");
        assert_eq!(metadata.version, "26.422.30944.2080-1");
        Ok(())
    }

    #[test]
    fn rejects_pacman_package_metadata_for_wrong_name() {
        let metadata = PacmanPackageMetadata {
            name: "other-app".to_string(),
            version: "26.422.30944.2080-1".to_string(),
        };

        let error = ensure_pacman_package_identity(
            &metadata,
            Path::new("/tmp/codex-app-26.422.30944.2080-1-x86_64.pkg.tar.zst"),
        )
        .expect_err("wrong pacman package name should be rejected");

        assert!(error.to_string().contains("codex-app"));
    }

    #[test]
    fn stages_install_candidate_to_private_copy() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("codex-app_26.422.30944.2080_amd64.deb");
        std::fs::write(&path, b"original")?;

        let staged = stage_install_candidate(&path, PackageKind::Deb)?;

        assert_ne!(staged.path, path);
        assert_eq!(std::fs::read(&staged.path)?, b"original");
        std::fs::write(&path, b"replaced")?;
        assert_eq!(std::fs::read(&staged.path)?, b"original");
        Ok(())
    }

    #[test]
    fn compares_debian_versions_using_dpkg_rules() -> Result<()> {
        if !program_exists(DPKG_CANDIDATES, "dpkg") {
            return Ok(());
        }

        assert!(is_version_newer(
            "2026.03.24.220000+88f07cd3",
            "2026.03.24.120000+afed8a8e"
        )?);
        assert!(!is_version_newer(
            "2026.03.24.120000+88f07cd3",
            "2026.03.24.120000+afed8a8e"
        )?);
        Ok(())
    }

    #[test]
    fn install_commands_require_a_file_name() {
        let deb_error = apt_install_command(Path::new("/")).expect_err("root is not a package");
        let rpm_error = dnf_install_command(Path::new("/")).expect_err("root is not a package");

        assert!(deb_error.to_string().contains("apt package path has no"));
        assert!(rpm_error.to_string().contains("dnf package path has no"));
    }

    #[test]
    fn empty_installed_version_output_is_reported_as_unknown() {
        assert_eq!(parse_installed_version(Vec::new()), "unknown");
    }

    #[test]
    fn parses_pacman_installed_version_output() {
        assert_eq!(
            parse_pacman_installed_version(b"codex-app 2026.04.02.120000-1\n".to_vec()),
            "2026.04.02.120000-1"
        );
    }

    #[test]
    fn parses_pacman_package_version_from_filename() -> Result<()> {
        assert_eq!(
            pacman_package_version(Path::new(
                "/tmp/codex-app-2026.04.02.120000-1-x86_64.pkg.tar.zst"
            ))?,
            "2026.04.02.120000-1"
        );
        Ok(())
    }
}
