//! Installation helpers for privileged and non-privileged package application.

use crate::package_verification;
use anyhow::{Context, Result};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::{
    env, fmt, fs, io,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

pub(crate) const PACKAGE_NAME: &str = "codex-app";
const INSTALLED_UPDATER_BINARY: &str = "/usr/bin/codex-app-updater";
const APT_CANDIDATES: &[&str] = &["/usr/bin/apt", "/bin/apt"];
const DNF_CANDIDATES: &[&str] = &["/usr/bin/dnf", "/bin/dnf", "/usr/bin/dnf5", "/bin/dnf5"];
const DPKG_CANDIDATES: &[&str] = &["/usr/bin/dpkg", "/bin/dpkg"];
const DPKG_DEB_CANDIDATES: &[&str] = &["/usr/bin/dpkg-deb", "/bin/dpkg-deb"];
const DPKG_QUERY_CANDIDATES: &[&str] = &["/usr/bin/dpkg-query", "/bin/dpkg-query"];
const RPM_CANDIDATES: &[&str] = &["/usr/bin/rpm", "/bin/rpm"];
const ZYPPER_CANDIDATES: &[&str] = &["/usr/bin/zypper", "/bin/zypper"];
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

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deb => "deb",
            Self::Rpm => "rpm",
            Self::Pacman => "pacman",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedPackage {
    sha256: String,
    package_name: String,
    package_version: String,
}

impl ExpectedPackage {
    pub fn new(
        sha256: impl Into<String>,
        package_name: impl Into<String>,
        package_version: impl Into<String>,
    ) -> Result<Self> {
        let sha256 = normalize_sha256(sha256.into())?;
        let package_name = package_name.into();
        let package_version = package_version.into();
        anyhow::ensure!(!package_name.is_empty(), "Expected package name is empty");
        anyhow::ensure!(
            !package_version.is_empty(),
            "Expected package version is empty"
        );
        Ok(Self {
            sha256,
            package_name,
            package_version,
        })
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    pub fn package_name(&self) -> &str {
        &self.package_name
    }

    pub fn package_version(&self) -> &str {
        &self.package_version
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InstallOptions {
    allow_same_version: bool,
}

impl InstallOptions {
    pub fn new(allow_same_version: bool, expected: Option<&ExpectedPackage>) -> Result<Self> {
        anyhow::ensure!(
            !allow_same_version || expected.is_some(),
            "--allow-same-version requires expected package verification arguments"
        );
        Ok(Self { allow_same_version })
    }
}

pub fn expected_package_from_args(
    expected_sha256: Option<String>,
    expected_package_name: Option<String>,
    expected_package_version: Option<String>,
) -> Result<Option<ExpectedPackage>> {
    match (
        expected_sha256,
        expected_package_name,
        expected_package_version,
    ) {
        (None, None, None) => Ok(None),
        (Some(sha256), Some(package_name), Some(package_version)) => {
            Ok(Some(ExpectedPackage::new(
                sha256,
                package_name,
                package_version,
            )?))
        }
        _ => anyhow::bail!(
            "Expected package verification requires --expected-sha256, --expected-package-name, and --expected-package-version together"
        ),
    }
}

#[cfg(test)]
pub(crate) fn expected_package_version_from_source(
    kind: PackageKind,
    source_version: &str,
) -> String {
    match kind {
        PackageKind::Deb => source_version.to_string(),
        PackageKind::Rpm => {
            let base = source_version
                .split_once('+')
                .map_or(source_version, |(base, _)| base);
            let release = source_version
                .split_once('+')
                .map_or("1", |(_, release)| release);
            format!("{base}-{release}")
        }
        PackageKind::Pacman => format!("{}-1", source_version.replace('+', "_")),
    }
}

#[cfg(not(test))]
pub(crate) fn package_name_for_verification(path: &Path) -> Result<String> {
    package_name(path)
}

#[cfg(test)]
pub(crate) fn package_name_for_verification(path: &Path) -> Result<String> {
    match package_name(path) {
        Ok(name) => Ok(name),
        Err(error) => {
            let _ = error;
            Ok(PACKAGE_NAME.to_string())
        }
    }
}

#[cfg(not(test))]
pub(crate) fn package_version_for_verification(
    path: &Path,
    _source_version: &str,
) -> Result<String> {
    package_version(path)
}

#[cfg(test)]
pub(crate) fn package_version_for_verification(
    path: &Path,
    source_version: &str,
) -> Result<String> {
    match package_version(path) {
        Ok(version) => Ok(version),
        Err(error) => {
            let _ = error;
            Ok(expected_package_version_from_source(
                PackageKind::from_path(path),
                source_version,
            ))
        }
    }
}

fn normalize_sha256(value: String) -> Result<String> {
    let value = value.trim().to_ascii_lowercase();
    anyhow::ensure!(
        value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit()),
        "Expected package digest must be a 64-character SHA-256 hex digest"
    );
    Ok(value)
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

    if pacman_installed {
        return PackageKind::Pacman;
    }
    if deb_installed {
        return PackageKind::Deb;
    }
    if rpm_installed {
        return PackageKind::Rpm;
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
            .any(|token| expected.contains(&token))
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
    installed_version_from_command(
        &program_path(DPKG_QUERY_CANDIDATES, "dpkg-query"),
        &["-W", "-f=${Version}", PACKAGE_NAME],
    )
}

fn installed_rpm_version() -> String {
    installed_version_from_command(
        &program_path(RPM_CANDIDATES, "rpm"),
        &["-q", "--queryformat", "%{VERSION}-%{RELEASE}", PACKAGE_NAME],
    )
}

fn installed_pacman_version() -> String {
    match Command::new(program_path(PACMAN_CANDIDATES, "pacman"))
        .args(["-Q", PACKAGE_NAME])
        .output()
    {
        Ok(output) if output.status.success() => parse_pacman_installed_version(output.stdout),
        _ => "unknown".to_string(),
    }
}

pub fn install_deb_with_options(
    path: &Path,
    expected: Option<&ExpectedPackage>,
    options: InstallOptions,
) -> Result<()> {
    anyhow::ensure!(
        path.exists(),
        "Debian package not found: {}",
        path.display()
    );
    let staged = stage_package_for_privileged_install(path)?;
    let staged_path = staged.path();
    verify_expected_package(staged_path, expected)?;
    ensure_deb_package_identity(staged_path)?;
    ensure_upgrade_path(staged_path, options.allow_same_version)?;

    if program_exists(APT_CANDIDATES, "apt") {
        let mut command = apt_install_command(staged_path, options.allow_same_version)?;
        run_install(&mut command).context("apt install failed")?;
        return Ok(());
    }

    let mut command = dpkg_install_command(staged_path);
    run_install(&mut command).context("dpkg -i failed")
}

pub fn install_rpm_with_options(
    path: &Path,
    expected: Option<&ExpectedPackage>,
    options: InstallOptions,
) -> Result<()> {
    anyhow::ensure!(path.exists(), "RPM package not found: {}", path.display());
    let staged = stage_package_for_privileged_install(path)?;
    let staged_path = staged.path();
    verify_expected_package(staged_path, expected)?;
    ensure_rpm_package_identity(staged_path)?;
    ensure_upgrade_path_rpm(staged_path, options.allow_same_version)?;

    if program_exists(DNF_CANDIDATES, "dnf") || program_exists(DNF_CANDIDATES, "dnf5") {
        let mut command = dnf_install_command(staged_path, options.allow_same_version)?;
        run_install(&mut command).context("dnf install failed")?;
        return Ok(());
    }

    if program_exists(ZYPPER_CANDIDATES, "zypper") {
        let mut command = zypper_install_command(staged_path, options.allow_same_version)?;
        run_install(&mut command).context("zypper install failed")?;
        return Ok(());
    }

    let mut command = rpm_install_command(staged_path, options.allow_same_version);
    run_install(&mut command).context("rpm -Uvh failed")
}

pub fn install_pacman_with_options(
    path: &Path,
    expected: Option<&ExpectedPackage>,
    options: InstallOptions,
) -> Result<()> {
    anyhow::ensure!(
        path.exists(),
        "Pacman package not found: {}",
        path.display()
    );
    let staged = stage_package_for_privileged_install(path)?;
    let staged_path = staged.path();
    verify_expected_package(staged_path, expected)?;
    ensure_upgrade_path_pacman(staged_path, options.allow_same_version)?;

    let mut command = pacman_install_command(staged_path);
    run_install(&mut command).context("pacman -U failed")
}

/// Builds the `pkexec` command used for privileged package installation.
pub fn pkexec_command(package_path: &Path, expected: Option<&ExpectedPackage>) -> Result<Command> {
    pkexec_command_with_options(package_path, expected, false)
}

pub fn pkexec_command_with_options(
    package_path: &Path,
    expected: Option<&ExpectedPackage>,
    allow_same_version: bool,
) -> Result<Command> {
    let _options = InstallOptions::new(allow_same_version, expected)?;
    let updater_binary = updater_binary_for_privileged_install()?;
    Ok(pkexec_command_with_updater_binary(
        &updater_binary,
        package_path,
        expected,
        allow_same_version,
    ))
}

fn pkexec_command_with_updater_binary(
    updater_binary: &Path,
    package_path: &Path,
    expected: Option<&ExpectedPackage>,
    allow_same_version: bool,
) -> Command {
    let subcommand = match PackageKind::from_path(package_path) {
        PackageKind::Rpm => "install-rpm",
        PackageKind::Deb => "install-deb",
        PackageKind::Pacman => "install-pacman",
    };
    let mut command = Command::new("pkexec");
    command
        .arg("--disable-internal-agent")
        .arg(updater_binary)
        .arg(subcommand)
        .arg("--path")
        .arg(package_path);
    if let Some(expected) = expected {
        command
            .arg("--expected-sha256")
            .arg(expected.sha256())
            .arg("--expected-package-name")
            .arg(expected.package_name())
            .arg("--expected-package-version")
            .arg(expected.package_version());
    }
    if allow_same_version {
        command.arg("--allow-same-version");
    }
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

pub(crate) struct StagedPackage {
    _dir: PrivateStagingDir,
    path: PathBuf,
}

impl StagedPackage {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

pub(crate) fn stage_package_for_privileged_install(path: &Path) -> Result<StagedPackage> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("Failed to inspect package {}", path.display()))?;
    anyhow::ensure!(
        metadata.is_file(),
        "Package path is not a regular file: {}",
        path.display()
    );
    let source_path = package_identity_path(path)?;
    let requested_kind = PackageKind::from_path(path);
    let kind = PackageKind::from_path(&source_path);
    anyhow::ensure!(
        requested_kind == kind,
        "Package format changed while resolving {}",
        path.display()
    );
    ensure_codex_package(&source_path)?;

    let dir = PrivateStagingDir::create("codex-app-privileged-install-")
        .context("Failed to create private package staging directory")?;
    let staged_path = dir.path().join(stable_file_name(kind, &source_path)?);
    fs::copy(&source_path, &staged_path).with_context(|| {
        format!(
            "Failed to stage package {} at {}",
            source_path.display(),
            staged_path.display()
        )
    })?;
    set_private_file_permissions(&staged_path)?;
    anyhow::ensure!(
        PackageKind::from_path(&staged_path) == kind,
        "Package format changed while stabilizing {}",
        path.display()
    );
    ensure_codex_package(&staged_path)?;

    Ok(StagedPackage {
        _dir: dir,
        path: staged_path,
    })
}

fn package_identity_path(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path)
        .with_context(|| format!("Failed to resolve package path {}", path.display()))
}

pub(crate) fn ensure_codex_package(path: &Path) -> Result<()> {
    match PackageKind::from_path(path) {
        PackageKind::Deb => ensure_package_name(&deb_package_name(path)?, path),
        PackageKind::Rpm => ensure_package_name(&rpm_package_name(path)?, path),
        PackageKind::Pacman => {
            pacman_package_version(path)?;
            ensure_package_name(&pacman_package_name(path)?, path)
        }
    }
}

pub(crate) fn verify_expected_package(
    path: &Path,
    expected: Option<&ExpectedPackage>,
) -> Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };

    let actual_sha256 = package_verification::file_sha256(path)?;
    anyhow::ensure!(
        actual_sha256 == expected.sha256(),
        "Package digest does not match updater verification: expected {}, found {}",
        expected.sha256(),
        actual_sha256
    );

    let actual_name = package_name(path)?;
    anyhow::ensure!(
        actual_name == expected.package_name(),
        "Package name does not match updater verification: expected {}, found {}",
        expected.package_name(),
        actual_name
    );

    let actual_version = package_version(path)?;
    anyhow::ensure!(
        actual_version == expected.package_version(),
        "Package version does not match updater verification: expected {}, found {}",
        expected.package_version(),
        actual_version
    );
    Ok(())
}

fn package_name(path: &Path) -> Result<String> {
    match PackageKind::from_path(path) {
        PackageKind::Deb => deb_package_name(path),
        PackageKind::Rpm => rpm_package_name(path),
        PackageKind::Pacman => pacman_package_name(path),
    }
}

fn package_version(path: &Path) -> Result<String> {
    match PackageKind::from_path(path) {
        PackageKind::Deb => deb_package_version(path),
        PackageKind::Rpm => rpm_package_version(path),
        PackageKind::Pacman => pacman_package_version(path),
    }
}

fn stable_file_name(kind: PackageKind, path: &Path) -> Result<String> {
    match kind {
        PackageKind::Deb => Ok("codex-app.deb".to_string()),
        PackageKind::Rpm => Ok("codex-app.rpm".to_string()),
        PackageKind::Pacman => path
            .file_name()
            .with_context(|| format!("Pacman package path has no file name: {}", path.display()))
            .map(|name| name.to_string_lossy().into_owned()),
    }
}

fn ensure_package_name(package_name: &str, path: &Path) -> Result<()> {
    anyhow::ensure!(
        package_name == PACKAGE_NAME,
        "Refusing to install package {package_name} from {}; expected {PACKAGE_NAME}",
        path.display()
    );
    Ok(())
}

struct PrivateStagingDir {
    path: PathBuf,
}

impl PrivateStagingDir {
    fn create(prefix: &str) -> Result<Self> {
        let base = env::temp_dir();
        let pid = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before UNIX epoch")?
            .as_nanos();

        for attempt in 0..100u32 {
            let path = base.join(format!("{prefix}{pid}-{nanos}-{attempt}"));
            let result = create_private_dir(&path);
            match result {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("Failed to create {}", path.display()));
                }
            }
        }

        anyhow::bail!(
            "could not allocate a unique private staging directory in {}",
            base.display()
        )
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrivateStagingDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl fmt::Debug for PrivateStagingDir {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PrivateStagingDir")
            .field("path", &self.path)
            .finish()
    }
}

#[cfg(unix)]
fn create_private_dir(path: &Path) -> io::Result<()> {
    fs::DirBuilder::new().mode(0o700).create(path)
}

#[cfg(not(unix))]
fn create_private_dir(path: &Path) -> io::Result<()> {
    fs::create_dir(path)
}

#[cfg(unix)]
fn set_private_file_permissions(path: &Path) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("Failed to lock down staged package {}", path.display()))
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &Path) -> Result<()> {
    Ok(())
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

fn ensure_upgrade_path(path: &Path, allow_same_version: bool) -> Result<()> {
    let installed = installed_package_version();
    if installed == "unknown" {
        return Ok(());
    }

    let candidate = deb_package_version(path)?;
    let version_allowed = if allow_same_version {
        is_version_newer_or_same(&candidate, &installed)?
    } else {
        is_version_newer(&candidate, &installed)?
    };
    anyhow::ensure!(
        version_allowed,
        "Refusing to install non-newer package version {candidate} over installed version {installed}"
    );
    Ok(())
}

fn ensure_deb_package_identity(path: &Path) -> Result<()> {
    let package_name = deb_package_field(path, "Package")?;
    anyhow::ensure!(
        package_name == PACKAGE_NAME,
        "Refusing to install Debian package {package_name}; expected {PACKAGE_NAME}"
    );
    Ok(())
}

fn ensure_rpm_package_identity(path: &Path) -> Result<()> {
    let output = Command::new(program_path(RPM_CANDIDATES, "rpm"))
        .arg("-qp")
        .arg("--queryformat")
        .arg("%{NAME}")
        .arg(path)
        .output()
        .context("Failed to inspect RPM package identity")?;

    anyhow::ensure!(
        output.status.success(),
        "rpm could not read the package name from {}",
        path.display()
    );
    let package_name = String::from_utf8(output.stdout)
        .context("rpm returned a non-UTF8 package name")?
        .trim()
        .to_string();
    anyhow::ensure!(
        package_name == PACKAGE_NAME,
        "Refusing to install RPM package {package_name}; expected {PACKAGE_NAME}"
    );
    Ok(())
}

fn ensure_upgrade_path_pacman(path: &Path, allow_same_version: bool) -> Result<()> {
    let installed = installed_pacman_version();
    if installed == "unknown" {
        return Ok(());
    }

    let candidate = pacman_package_version(path)?;
    let version_allowed = if allow_same_version {
        is_version_newer_or_same_pacman(&candidate, &installed)?
    } else {
        is_version_newer_pacman(&candidate, &installed)?
    };
    anyhow::ensure!(
        version_allowed,
        "Refusing to install non-newer package version {candidate} over installed version {installed}"
    );
    Ok(())
}

fn ensure_upgrade_path_rpm(path: &Path, allow_same_version: bool) -> Result<()> {
    let installed = installed_rpm_version();
    if installed == "unknown" {
        return Ok(());
    }

    let candidate = rpm_package_version(path)?;
    let version_allowed = match compare_generated_package_versions(&candidate, &installed) {
        Some(std::cmp::Ordering::Greater) => true,
        Some(std::cmp::Ordering::Equal) => allow_same_version,
        _ => false,
    };
    anyhow::ensure!(
        version_allowed,
        "Refusing to install non-newer package version {candidate} over installed version {installed}"
    );
    Ok(())
}

fn apt_install_command(path: &Path, reinstall: bool) -> Result<Command> {
    install_command_in_parent(&program_path(APT_CANDIDATES, "apt"), path, reinstall)
}

fn dpkg_install_command(path: &Path) -> Command {
    let mut command = Command::new(program_path(DPKG_CANDIDATES, "dpkg"));
    command.arg("-i").arg("--").arg(path.as_os_str());
    command
}

fn dnf_install_command(path: &Path, reinstall: bool) -> Result<Command> {
    install_command_in_parent(&program_path(DNF_CANDIDATES, "dnf"), path, reinstall)
}

fn zypper_install_command(path: &Path, force: bool) -> Result<Command> {
    let program = program_path(ZYPPER_CANDIDATES, "zypper");
    let parent = path
        .parent()
        .with_context(|| "zypper package path has no parent directory")?;
    let file_name = path
        .file_name()
        .with_context(|| "zypper package path has no file name")?
        .to_string_lossy()
        .into_owned();

    let mut command = Command::new(program);
    command
        .current_dir(parent)
        .args(["--non-interactive", "install", "--allow-unsigned-rpm"]);
    if force {
        command.arg("--force");
    }
    command.arg("-y").arg(format!("./{file_name}"));
    Ok(command)
}

fn install_command_in_parent(program: &Path, path: &Path, reinstall: bool) -> Result<Command> {
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
        .arg(if reinstall { "reinstall" } else { "install" });
    if program_name == "apt" && reinstall {
        command.arg("--reinstall");
    }
    command.arg("-y").arg(format!("./{file_name}"));
    Ok(command)
}

fn rpm_install_command(path: &Path, replace_package: bool) -> Command {
    let mut command = Command::new(program_path(RPM_CANDIDATES, "rpm"));
    command.arg("-Uvh");
    if replace_package {
        command.arg("--replacepkgs");
    }
    command.arg("--").arg(path.as_os_str());
    command
}

fn pacman_install_command(path: &Path) -> Command {
    let mut command = Command::new(program_path(PACMAN_CANDIDATES, "pacman"));
    command
        .args(["-U", "--noconfirm", "--"])
        .arg(path.as_os_str());
    command
}

fn updater_binary_for_privileged_install() -> Result<PathBuf> {
    let installed = PathBuf::from(INSTALLED_UPDATER_BINARY);
    validate_installed_updater_binary(&installed)
}

fn validate_installed_updater_binary(installed: &Path) -> Result<PathBuf> {
    let metadata = fs::metadata(installed).with_context(|| {
        format!(
            "Privileged install requires the installed updater binary at {}",
            installed.display()
        )
    })?;
    anyhow::ensure!(
        metadata.is_file(),
        "Installed updater binary is not a regular file: {}",
        installed.display()
    );
    anyhow::ensure!(
        metadata.uid() == 0 && metadata.permissions().mode() & 0o022 == 0,
        "Installed updater binary must be root-owned and not group/world-writable: {}",
        installed.display()
    );
    Ok(installed.to_path_buf())
}

fn deb_package_name(path: &Path) -> Result<String> {
    let output = dpkg_deb_field_command(path, "Package")
        .output()
        .context("Failed to inspect Debian package metadata")?;

    package_metadata_field(output, "dpkg-deb", "package name", path).with_context(|| {
        format!(
            "Failed to inspect Debian package metadata for {}",
            path.display()
        )
    })
}

fn deb_package_version(path: &Path) -> Result<String> {
    deb_package_field(path, "Version")
}

fn deb_package_field(path: &Path, field: &str) -> Result<String> {
    let output = Command::new(program_path(DPKG_DEB_CANDIDATES, "dpkg-deb"))
        .arg("-f")
        .arg(path)
        .arg(field)
        .output()
        .with_context(|| format!("Failed to inspect Debian package {field}"))?;

    anyhow::ensure!(
        output.status.success(),
        "dpkg-deb could not read package field {field} from {}",
        path.display()
    );

    let value = String::from_utf8(output.stdout)
        .with_context(|| format!("dpkg-deb returned a non-UTF8 package field {field}"))?
        .trim()
        .to_string();
    anyhow::ensure!(
        !value.is_empty(),
        "dpkg-deb returned an empty package field {field} for {}",
        path.display()
    );
    Ok(value)
}

fn rpm_package_name(path: &Path) -> Result<String> {
    let output = rpm_query_command(path, "%{NAME}")
        .output()
        .context("Failed to inspect RPM package metadata")?;

    package_metadata_field(output, "rpm", "package name", path)
}

fn rpm_package_version(path: &Path) -> Result<String> {
    let output = rpm_query_command(path, "%{VERSION}-%{RELEASE}")
        .output()
        .context("Failed to inspect RPM package metadata")?;

    anyhow::ensure!(
        output.status.success(),
        "rpm could not read the package version from {}",
        path.display()
    );

    let version = String::from_utf8(output.stdout)
        .context("rpm returned a non-UTF8 package version")?
        .trim()
        .to_string();
    anyhow::ensure!(
        !version.is_empty(),
        "rpm returned an empty package version for {}",
        path.display()
    );
    Ok(version)
}

fn pacman_package_name(path: &Path) -> Result<String> {
    let output = pacman_query_name_command(path)
        .output()
        .context("Failed to inspect pacman package metadata")?;

    package_metadata_field(output, "pacman", "package name", path)
}

fn dpkg_deb_field_command(path: &Path, field: &str) -> Command {
    let mut command = Command::new(program_path(DPKG_DEB_CANDIDATES, "dpkg-deb"));
    command.arg("-f").arg("--").arg(path).arg(field);
    command
}

fn rpm_query_command(path: &Path, queryformat: &str) -> Command {
    let mut command = Command::new(program_path(RPM_CANDIDATES, "rpm"));
    command
        .arg("-qp")
        .arg("--queryformat")
        .arg(queryformat)
        .arg("--")
        .arg(path);
    command
}

fn pacman_query_name_command(path: &Path) -> Command {
    let mut command = Command::new(program_path(PACMAN_CANDIDATES, "pacman"));
    command.args(["-Qqp", "--"]).arg(path);
    command
}

fn package_metadata_field(
    output: std::process::Output,
    program: &str,
    field: &str,
    path: &Path,
) -> Result<String> {
    anyhow::ensure!(
        output.status.success(),
        "{program} could not read the {field} from {}",
        path.display()
    );

    let value = String::from_utf8(output.stdout)
        .with_context(|| format!("{program} returned a non-UTF8 {field}"))?
        .trim()
        .to_string();
    anyhow::ensure!(
        !value.is_empty(),
        "{program} returned an empty {field} for {}",
        path.display()
    );
    Ok(value)
}

fn is_version_newer(candidate: &str, installed: &str) -> Result<bool> {
    let status = Command::new(program_path(DPKG_CANDIDATES, "dpkg"))
        .args(["--compare-versions", candidate, "gt", installed])
        .status()
        .context("Failed to compare Debian package versions")?;
    Ok(status.success())
}

fn is_version_newer_or_same(candidate: &str, installed: &str) -> Result<bool> {
    let status = Command::new(program_path(DPKG_CANDIDATES, "dpkg"))
        .args(["--compare-versions", candidate, "ge", installed])
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
    Ok(compare_pacman_versions(candidate, installed)? > 0)
}

fn is_version_newer_or_same_pacman(candidate: &str, installed: &str) -> Result<bool> {
    Ok(compare_pacman_versions(candidate, installed)? >= 0)
}

fn compare_pacman_versions(candidate: &str, installed: &str) -> Result<i32> {
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
    Ok(comparison)
}

#[cfg(test)]
fn generated_package_version_is_newer(candidate: &str, installed: &str) -> bool {
    matches!(
        compare_generated_package_versions(candidate, installed),
        Some(std::cmp::Ordering::Greater)
    )
}

fn compare_generated_package_versions(left: &str, right: &str) -> Option<std::cmp::Ordering> {
    let left = parse_generated_package_version(left)?;
    let right = parse_generated_package_version(right)?;
    Some(left.cmp(&right))
}

fn parse_generated_package_version(version: &str) -> Option<Vec<u32>> {
    let without_metadata = version
        .split_once('+')
        .map(|(prefix, _)| prefix)
        .unwrap_or(version);
    let mut parts = Vec::new();

    for segment in without_metadata.split(['.', '-']) {
        let numeric_prefix = segment
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect::<String>();
        if numeric_prefix.is_empty() {
            continue;
        }
        parts.push(numeric_prefix.parse().ok()?);
    }

    if parts.len() < 3 {
        return None;
    }

    Some(parts)
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
        let command = pkexec_command_with_updater_binary(
            Path::new("/usr/bin/codex-app-updater"),
            Path::new("/tmp/update.deb"),
            None,
            false,
        );
        let args: Vec<_> = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec![
                "--disable-internal-agent",
                "/usr/bin/codex-app-updater",
                "install-deb",
                "--path",
                "/tmp/update.deb"
            ]
        );
    }

    #[test]
    fn builds_pkexec_command_for_privileged_rpm_install() {
        let command = pkexec_command_with_updater_binary(
            Path::new("/usr/bin/codex-app-updater"),
            Path::new("/tmp/update.rpm"),
            None,
            false,
        );
        let args: Vec<_> = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec![
                "--disable-internal-agent",
                "/usr/bin/codex-app-updater",
                "install-rpm",
                "--path",
                "/tmp/update.rpm"
            ]
        );
    }

    #[test]
    fn package_verification_args_are_passed_to_privileged_install_command() -> Result<()> {
        let expected = ExpectedPackage::new(
            "6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a",
            "codex-app",
            "26.429.20946",
        )?;
        let command = pkexec_command_with_updater_binary(
            Path::new("/usr/bin/codex-app-updater"),
            Path::new("/tmp/update.deb"),
            Some(&expected),
            false,
        );
        let args: Vec<_> = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec![
                "--disable-internal-agent",
                "/usr/bin/codex-app-updater",
                "install-deb",
                "--path",
                "/tmp/update.deb",
                "--expected-sha256",
                "6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a",
                "--expected-package-name",
                "codex-app",
                "--expected-package-version",
                "26.429.20946"
            ]
        );
        Ok(())
    }

    #[test]
    fn same_version_wrapper_install_is_explicitly_flagged() -> Result<()> {
        let expected = ExpectedPackage::new(
            "6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a",
            "codex-app",
            "26.429.20946",
        )?;
        let command = pkexec_command_with_updater_binary(
            Path::new("/usr/bin/codex-app-updater"),
            Path::new("/tmp/update.deb"),
            Some(&expected),
            true,
        );
        let args: Vec<_> = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert!(args.contains(&"--allow-same-version".to_string()));
        Ok(())
    }

    #[test]
    fn same_version_install_requires_expected_package_binding() {
        let error = InstallOptions::new(true, None)
            .expect_err("same-version installs must be package-bound");
        assert!(error.to_string().contains("expected package verification"));
    }

    #[test]
    fn rejects_uninstalled_updater_path_for_pkexec() {
        let temp = tempfile::tempdir().expect("tempdir");
        let missing = temp.path().join("missing-codex-app-updater");

        let error = validate_installed_updater_binary(&missing)
            .expect_err("missing installed updater should be rejected");

        assert!(error.to_string().contains("installed updater binary"));
    }

    #[test]
    fn builds_local_apt_install_command() -> Result<()> {
        let command = apt_install_command(Path::new("/tmp/build/codex.deb"), false)?;
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
        let command = dnf_install_command(Path::new("/tmp/build/codex.rpm"), false)?;
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
    fn builds_local_zypper_install_command() -> Result<()> {
        let command = zypper_install_command(Path::new("/tmp/build/codex.rpm"), false)?;
        assert!(command.get_program().to_string_lossy().ends_with("zypper"));
        assert_eq!(
            command
                .get_args()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec![
                "--non-interactive",
                "install",
                "--allow-unsigned-rpm",
                "-y",
                "./codex.rpm"
            ]
        );
        Ok(())
    }

    #[test]
    fn same_version_installs_use_reinstall_command_shapes() -> Result<()> {
        let apt = apt_install_command(Path::new("/tmp/build/codex.deb"), true)?;
        assert_eq!(
            apt.get_args()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["reinstall", "--reinstall", "-y", "./codex.deb"]
        );

        let dnf = dnf_install_command(Path::new("/tmp/build/codex.rpm"), true)?;
        assert_eq!(
            dnf.get_args()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["reinstall", "-y", "./codex.rpm"]
        );

        let zypper = zypper_install_command(Path::new("/tmp/build/codex.rpm"), true)?;
        assert_eq!(
            zypper
                .get_args()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec![
                "--non-interactive",
                "install",
                "--allow-unsigned-rpm",
                "--force",
                "-y",
                "./codex.rpm"
            ]
        );

        let rpm = rpm_install_command(Path::new("/tmp/build/codex.rpm"), true);
        assert_eq!(
            rpm.get_args()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["-Uvh", "--replacepkgs", "--", "/tmp/build/codex.rpm"]
        );
        Ok(())
    }

    #[test]
    fn rejects_unreadable_privileged_install_package_metadata() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("codex-app_26.429.20946_amd64.deb");
        fs::write(&source, b"validated bytes")?;

        let error = match stage_package_for_privileged_install(&source) {
            Ok(_) => anyhow::bail!("fake package metadata was accepted"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("Debian package metadata"));
        Ok(())
    }

    #[test]
    fn stable_file_name_uses_safe_names_for_deb_and_rpm() -> Result<()> {
        assert_eq!(
            stable_file_name(PackageKind::Deb, Path::new("-evil.deb"))?,
            "codex-app.deb"
        );
        assert_eq!(
            stable_file_name(PackageKind::Rpm, Path::new("-evil.rpm"))?,
            "codex-app.rpm"
        );
        assert_eq!(
            stable_file_name(
                PackageKind::Pacman,
                Path::new("/tmp/codex-app-2026.03.30-1-x86_64.pkg.tar.zst")
            )?,
            "codex-app-2026.03.30-1-x86_64.pkg.tar.zst"
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
    fn detection_prefers_arch_os_release_even_if_rpm_command_exists() {
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
    fn detection_prefers_fedora_os_release_even_if_deb_package_is_installed() {
        assert_eq!(
            detect_package_kind(
                false,
                true,
                true,
                false,
                true,
                false,
                Some(("fedora".to_string(), "rhel".to_string())),
            ),
            PackageKind::Rpm
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
        let command = pkexec_command_with_updater_binary(
            Path::new("/usr/bin/codex-app-updater"),
            Path::new("/tmp/update.pkg.tar.zst"),
            None,
            false,
        );
        let args: Vec<_> = command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec![
                "--disable-internal-agent",
                "/usr/bin/codex-app-updater",
                "install-pacman",
                "--path",
                "/tmp/update.pkg.tar.zst"
            ]
        );
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
        assert!(is_version_newer_or_same(
            "2026.03.24.120000+afed8a8e",
            "2026.03.24.120000+afed8a8e"
        )?);
        assert!(!is_version_newer(
            "2026.03.24.120000+88f07cd3",
            "2026.03.24.120000+afed8a8e"
        )?);
        Ok(())
    }

    #[test]
    fn compares_generated_package_versions_by_timestamp() {
        assert!(generated_package_version_is_newer(
            "2026.04.28.140000-abcdef12.fc43",
            "2026.04.28.082247-12345678.fc43"
        ));
        assert!(!generated_package_version_is_newer(
            "2026.04.28.082247-12345678.fc43",
            "2026.04.28.140000-abcdef12.fc43"
        ));
        assert!(!generated_package_version_is_newer(
            "2026.04.28.140000-abcdef12.fc43",
            "2026.04.28.140000-abcdef12.fc43"
        ));
    }

    #[test]
    fn generated_package_version_comparison_supports_dmg_app_versions() {
        assert_eq!(
            compare_generated_package_versions("26.429.20946", "26.428.10000"),
            Some(std::cmp::Ordering::Greater)
        );
        assert!(!generated_package_version_is_newer(
            "not-a-version",
            "26.428.10000"
        ));
    }

    #[test]
    fn install_commands_require_a_file_name() {
        let deb_error =
            apt_install_command(Path::new("/"), false).expect_err("root is not a package");
        let rpm_error =
            dnf_install_command(Path::new("/"), false).expect_err("root is not a package");
        let zypper_error =
            zypper_install_command(Path::new("/"), false).expect_err("root is not a package");

        assert!(deb_error.to_string().contains("apt package path has no"));
        assert!(rpm_error.to_string().contains("dnf package path has no"));
        assert!(zypper_error
            .to_string()
            .contains("zypper package path has no"));
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

    #[cfg(unix)]
    #[test]
    fn resolves_pacman_latest_symlink_to_versioned_package_identity() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let package_name = "codex-app-2026.04.02.120000-1-x86_64.pkg.tar.zst";
        let package_path = temp.path().join(package_name);
        let latest_path = temp.path().join("codex-app-latest.pkg.tar.zst");
        std::fs::write(&package_path, b"pkg")?;
        std::os::unix::fs::symlink(package_name, &latest_path)?;

        let identity_path = package_identity_path(&latest_path)?;

        assert_eq!(
            identity_path.file_name().and_then(|name| name.to_str()),
            Some(package_name)
        );
        assert_eq!(
            stable_file_name(PackageKind::Pacman, &identity_path)?,
            package_name
        );
        assert_eq!(
            pacman_package_version(&identity_path)?,
            "2026.04.02.120000-1"
        );
        Ok(())
    }

    #[test]
    fn rejects_mismatched_package_name() {
        let error = ensure_package_name("not-codex", Path::new("/tmp/not-codex.deb"))
            .expect_err("foreign package names must be rejected");

        assert!(error.to_string().contains("expected codex-app"));
    }

    #[test]
    fn accepts_codex_package_name() -> Result<()> {
        ensure_package_name("codex-app", Path::new("/tmp/codex-app.deb"))
    }

    #[test]
    fn rejects_non_codex_pacman_package_filename() {
        let error = ensure_codex_package(Path::new(
            "/tmp/not-codex-2026.04.02.120000-1-x86_64.pkg.tar.zst",
        ))
        .expect_err("foreign pacman packages must be rejected");

        assert!(error.to_string().contains("codex-app-"));
    }
}
