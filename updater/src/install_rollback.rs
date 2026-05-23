//! Explicit rollback package installation helpers.

use crate::install::{stage_package_for_privileged_install, PackageKind};
use anyhow::{Context, Result};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};

const PACKAGE_NAME: &str = "codex-app";
const INSTALLED_UPDATER_BINARY: &str = "/usr/bin/codex-app-updater";
const APT_CANDIDATES: &[&str] = &["/usr/bin/apt", "/bin/apt"];
const DNF_CANDIDATES: &[&str] = &["/usr/bin/dnf", "/bin/dnf", "/usr/bin/dnf5", "/bin/dnf5"];
const DPKG_CANDIDATES: &[&str] = &["/usr/bin/dpkg", "/bin/dpkg"];
const DPKG_DEB_CANDIDATES: &[&str] = &["/usr/bin/dpkg-deb", "/bin/dpkg-deb"];
const RPM_CANDIDATES: &[&str] = &["/usr/bin/rpm", "/bin/rpm"];
const ZYPPER_CANDIDATES: &[&str] = &["/usr/bin/zypper", "/bin/zypper"];
const PACMAN_CANDIDATES: &[&str] = &["/usr/bin/pacman", "/bin/pacman"];
const PACMAN_PACKAGE_SUFFIXES: &[&str] = &[
    ".pkg.tar.zst",
    ".pkg.tar.xz",
    ".pkg.tar.gz",
    ".pkg.tar.bz2",
    ".pkg.tar.lz",
    ".pkg.tar.lz4",
    ".pkg.tar.lz5",
];

pub fn install_deb(path: &Path) -> Result<()> {
    anyhow::ensure!(
        path.exists(),
        "Debian rollback package not found: {}",
        path.display()
    );
    let staged = stage_package_for_privileged_install(path)?;
    let staged_path = staged.path();
    ensure_deb_package_identity(staged_path)?;

    if program_exists(APT_CANDIDATES, "apt") {
        let mut command = apt_command(staged_path)?;
        run_install(&mut command).context("apt rollback install failed")?;
        return Ok(());
    }

    let mut command = dpkg_command(staged_path);
    run_install(&mut command).context("dpkg rollback install failed")
}

pub fn install_rpm(path: &Path) -> Result<()> {
    anyhow::ensure!(
        path.exists(),
        "RPM rollback package not found: {}",
        path.display()
    );
    let staged = stage_package_for_privileged_install(path)?;
    let staged_path = staged.path();
    ensure_rpm_package_identity(staged_path)?;

    if let Some(dnf) = first_available_program(DNF_CANDIDATES, &["dnf", "dnf5"]) {
        let mut command = dnf_command(&dnf, staged_path)?;
        run_install(&mut command).context("dnf rollback install failed")?;
        return Ok(());
    }

    if program_exists(ZYPPER_CANDIDATES, "zypper") {
        let mut command = zypper_command(staged_path)?;
        run_install(&mut command).context("zypper rollback install failed")?;
        return Ok(());
    }

    let mut command = rpm_command(staged_path);
    run_install(&mut command).context("rpm rollback install failed")
}

pub fn install_pacman(path: &Path) -> Result<()> {
    anyhow::ensure!(
        path.exists(),
        "Pacman rollback package not found: {}",
        path.display()
    );
    let staged = stage_package_for_privileged_install(path)?;
    let staged_path = staged.path();
    ensure_pacman_package_identity(staged_path)?;

    let mut command = pacman_command(staged_path);
    run_install(&mut command).context("pacman rollback install failed")
}

pub fn pkexec_command(package_path: &Path) -> Result<Command> {
    let updater_binary = updater_binary_for_privileged_install()?;
    Ok(pkexec_command_with_updater_binary(
        &updater_binary,
        package_path,
    ))
}

fn pkexec_command_with_updater_binary(updater_binary: &Path, package_path: &Path) -> Command {
    let subcommand = match PackageKind::from_path(package_path) {
        PackageKind::Rpm => "install-rollback-rpm",
        PackageKind::Deb => "install-rollback-deb",
        PackageKind::Pacman => "install-rollback-pacman",
    };
    let mut command = Command::new("pkexec");
    command
        .arg("--disable-internal-agent")
        .arg(updater_binary)
        .arg(subcommand)
        .arg("--path")
        .arg(package_path);
    command
}

fn run_install(command: &mut Command) -> Result<()> {
    let status = command
        .status()
        .context("Failed to execute rollback installation command")?;
    anyhow::ensure!(
        status.success(),
        "rollback installation command exited with {status}"
    );
    Ok(())
}

fn apt_command(path: &Path) -> Result<Command> {
    let parent = package_parent(path, "apt rollback")?;
    let file_name = package_file_name(path, "apt rollback")?;
    let mut command = Command::new(program_path(APT_CANDIDATES, "apt"));
    command
        .current_dir(parent)
        .args(["install", "-y", "--allow-downgrades"])
        .arg(format!("./{file_name}"));
    Ok(command)
}

fn dpkg_command(path: &Path) -> Command {
    let mut command = Command::new(program_path(DPKG_CANDIDATES, "dpkg"));
    command.arg("-i").arg("--").arg(path.as_os_str());
    command
}

fn dnf_command(program: &Path, path: &Path) -> Result<Command> {
    command_in_parent(program, path, "downgrade")
}

fn zypper_command(path: &Path) -> Result<Command> {
    let parent = package_parent(path, "zypper rollback")?;
    let file_name = package_file_name(path, "zypper rollback")?;
    let mut command = Command::new(program_path(ZYPPER_CANDIDATES, "zypper"));
    command
        .current_dir(parent)
        .args([
            "--non-interactive",
            "install",
            "--allow-unsigned-rpm",
            "--oldpackage",
            "-y",
        ])
        .arg(format!("./{file_name}"));
    Ok(command)
}

fn rpm_command(path: &Path) -> Command {
    let mut command = Command::new(program_path(RPM_CANDIDATES, "rpm"));
    command
        .args(["-Uvh", "--oldpackage", "--"])
        .arg(path.as_os_str());
    command
}

fn pacman_command(path: &Path) -> Command {
    let mut command = Command::new(program_path(PACMAN_CANDIDATES, "pacman"));
    command
        .args(["-U", "--noconfirm", "--"])
        .arg(path.as_os_str());
    command
}

fn ensure_deb_package_identity(path: &Path) -> Result<()> {
    let output = Command::new(program_path(DPKG_DEB_CANDIDATES, "dpkg-deb"))
        .arg("-f")
        .arg(path)
        .arg("Package")
        .output()
        .context("Failed to inspect Debian rollback package identity")?;
    anyhow::ensure!(
        output.status.success(),
        "dpkg-deb could not read the package name from {}",
        path.display()
    );
    let package_name = String::from_utf8(output.stdout)
        .context("dpkg-deb returned a non-UTF8 package name")?
        .trim()
        .to_string();
    anyhow::ensure!(
        package_name == PACKAGE_NAME,
        "Refusing to roll back Debian package {package_name}; expected {PACKAGE_NAME}"
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
        .context("Failed to inspect RPM rollback package identity")?;
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
        "Refusing to roll back RPM package {package_name}; expected {PACKAGE_NAME}"
    );
    Ok(())
}

fn ensure_pacman_package_identity(path: &Path) -> Result<()> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .with_context(|| {
            format!(
                "Pacman rollback package path has no file name: {}",
                path.display()
            )
        })?;
    let stripped = strip_pacman_package_suffix(file_name)
        .with_context(|| format!("Not a valid pacman rollback package filename: {file_name}"))?;
    anyhow::ensure!(
        stripped.starts_with(&format!("{PACKAGE_NAME}-")),
        "Refusing to roll back pacman package {file_name}; expected {PACKAGE_NAME}"
    );
    let output = Command::new(program_path(PACMAN_CANDIDATES, "pacman"))
        .arg("-Qip")
        .arg(path)
        .output()
        .context("Failed to inspect pacman rollback package identity")?;
    anyhow::ensure!(
        output.status.success(),
        "pacman could not read the package metadata from {}",
        path.display()
    );
    let package_name = parse_pacman_package_name(&output.stdout)
        .context("pacman package metadata has no Name field")?;
    anyhow::ensure!(
        package_name == PACKAGE_NAME,
        "Refusing to roll back pacman package {package_name}; expected {PACKAGE_NAME}"
    );
    Ok(())
}

fn command_in_parent(program: &Path, path: &Path, verb: &str) -> Result<Command> {
    let program_name = program
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("package manager");
    let parent = package_parent(path, program_name)?;
    let file_name = package_file_name(path, program_name)?;

    let mut command = Command::new(program);
    command
        .current_dir(parent)
        .arg(verb)
        .arg("-y")
        .arg(format!("./{file_name}"));
    Ok(command)
}

fn package_parent<'a>(path: &'a Path, label: &str) -> Result<&'a Path> {
    path.parent()
        .with_context(|| format!("{label} package path has no parent directory"))
}

fn package_file_name(path: &Path, label: &str) -> Result<String> {
    Ok(path
        .file_name()
        .with_context(|| format!("{label} package path has no file name"))?
        .to_string_lossy()
        .into_owned())
}

fn updater_binary_for_privileged_install() -> Result<PathBuf> {
    let installed = PathBuf::from(INSTALLED_UPDATER_BINARY);
    let metadata = fs::metadata(&installed).with_context(|| {
        format!(
            "Privileged rollback requires the installed updater binary at {}",
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
    Ok(installed)
}

fn program_path(candidates: &[&str], fallback: &str) -> PathBuf {
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from(fallback))
}

fn first_available_program(candidates: &[&str], names: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
        .or_else(|| names.iter().find_map(command_path))
}

fn program_exists(candidates: &[&str], name: &str) -> bool {
    candidates.iter().map(Path::new).any(|path| path.is_file()) || command_exists(name)
}

fn command_path(name: &&str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|entry| entry.join(name))
            .find(|candidate| candidate.is_file())
    })
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

fn strip_pacman_package_suffix(file_name: &str) -> Option<&str> {
    let lower = file_name.to_ascii_lowercase();
    PACMAN_PACKAGE_SUFFIXES.iter().find_map(|suffix| {
        lower
            .strip_suffix(suffix)
            .map(|_| &file_name[..file_name.len() - suffix.len()])
    })
}

fn parse_pacman_package_name(stdout: &[u8]) -> Option<String> {
    String::from_utf8_lossy(stdout).lines().find_map(|line| {
        line.strip_prefix("Name")
            .and_then(|rest| rest.split_once(':'))
            .map(|(_, value)| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_local_apt_rollback_command() -> Result<()> {
        let command = apt_command(Path::new("/tmp/build/codex.deb"))?;
        assert!(command.get_program().to_string_lossy().ends_with("apt"));
        assert_eq!(
            command
                .get_args()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["install", "-y", "--allow-downgrades", "./codex.deb"]
        );
        Ok(())
    }

    #[test]
    fn builds_local_dnf_rollback_command() -> Result<()> {
        let command = dnf_command(
            Path::new("/usr/bin/dnf5"),
            Path::new("/tmp/build/codex.rpm"),
        )?;
        let program = command.get_program().to_string_lossy();
        assert!(program.ends_with("dnf") || program.ends_with("dnf5"));
        assert_eq!(
            command
                .get_args()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["downgrade", "-y", "./codex.rpm"]
        );
        Ok(())
    }

    #[test]
    fn builds_local_zypper_rollback_command() -> Result<()> {
        let command = zypper_command(Path::new("/tmp/build/codex.rpm"))?;
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
                "--oldpackage",
                "-y",
                "./codex.rpm"
            ]
        );
        Ok(())
    }

    #[test]
    fn direct_rollback_commands_stop_option_parsing() {
        assert_eq!(
            command_args(dpkg_command(Path::new("-evil.deb"))),
            vec!["-i", "--", "-evil.deb"]
        );
        assert_eq!(
            command_args(rpm_command(Path::new("-evil.rpm"))),
            vec!["-Uvh", "--oldpackage", "--", "-evil.rpm"]
        );
        assert_eq!(
            command_args(pacman_command(Path::new("-evil.pkg.tar.zst"))),
            vec!["-U", "--noconfirm", "--", "-evil.pkg.tar.zst"]
        );
    }

    fn command_args(command: Command) -> Vec<String> {
        command
            .get_args()
            .map(|value| value.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn builds_pkexec_command_for_privileged_rollback() {
        let command = pkexec_command_with_updater_binary(
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
                "--disable-internal-agent",
                "/usr/bin/codex-app-updater",
                "install-rollback-rpm",
                "--path",
                "/tmp/update.rpm"
            ]
        );
    }

    #[test]
    fn parses_pacman_package_name() {
        let metadata = b"Name            : codex-app\nVersion         : 1-1\n";
        assert_eq!(
            parse_pacman_package_name(metadata).as_deref(),
            Some("codex-app")
        );
    }
}
