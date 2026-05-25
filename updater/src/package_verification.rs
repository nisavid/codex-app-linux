//! Package digest and identity binding for updater-managed installs.

use crate::{
    install::{self, ExpectedPackage, PackageKind},
    state::PackageVerification,
};
use anyhow::{Context, Result};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};

pub fn record_built_package(
    package_path: &Path,
    workspace_dir: &Path,
    candidate_version: &str,
    dmg_sha256: &str,
) -> Result<PackageVerification> {
    let package_path = fs::canonicalize(package_path)
        .with_context(|| format!("Failed to resolve package path {}", package_path.display()))?;
    let workspace_dir = fs::canonicalize(workspace_dir).with_context(|| {
        format!(
            "Failed to resolve updater workspace {}",
            workspace_dir.display()
        )
    })?;
    anyhow::ensure!(
        package_path.starts_with(&workspace_dir),
        "Updater-built package is outside its workspace: {}",
        package_path.display()
    );

    let package_kind = PackageKind::from_path(&package_path);
    Ok(PackageVerification {
        package_kind: package_kind.as_str().to_string(),
        package_path: package_path.clone(),
        workspace_dir,
        package_name: install::package_name_for_verification(&package_path)?,
        package_version: install::package_version_for_verification(
            &package_path,
            candidate_version,
        )?,
        sha256: file_sha256(&package_path)?,
        candidate_version: candidate_version.to_string(),
        dmg_sha256: dmg_sha256.to_string(),
        verified_at: Utc::now(),
    })
}

pub fn expected_package_for_ready_install(
    package_path: &Path,
    workspace_root: &Path,
    candidate_version: Option<&str>,
    dmg_sha256: Option<&str>,
    verification: Option<&PackageVerification>,
) -> Result<ExpectedPackage> {
    let verification = verification
        .context("Ready update package verification is missing; run a new update check to rebuild the package")?;
    let candidate_version = candidate_version
        .context("Ready update package verification is missing a candidate version")?;
    let dmg_sha256 =
        dmg_sha256.context("Ready update package verification is missing a DMG digest")?;

    anyhow::ensure!(
        verification.candidate_version == candidate_version,
        "Ready update package verification does not match the candidate version"
    );
    anyhow::ensure!(
        verification.dmg_sha256 == dmg_sha256,
        "Ready update package verification does not match the trusted DMG digest"
    );
    verify_package_binding("Ready update", package_path, workspace_root, verification)?;
    ExpectedPackage::new(
        verification.sha256.clone(),
        verification.package_name.clone(),
        verification.package_version.clone(),
    )
}

pub fn expected_package_for_rollback(
    package_path: &Path,
    workspace_root: &Path,
    verification: Option<&PackageVerification>,
) -> Result<ExpectedPackage> {
    let verification = verification.context(
        "Rollback package verification is missing; run a new update check before rollback",
    )?;
    verify_package_binding("Rollback", package_path, workspace_root, verification)?;
    ExpectedPackage::new(
        verification.sha256.clone(),
        verification.package_name.clone(),
        verification.package_version.clone(),
    )
}

pub fn verification_matches_package_path(
    verification: &PackageVerification,
    package_path: &Path,
) -> bool {
    fs::canonicalize(package_path)
        .map(|package_path| package_path == verification.package_path)
        .unwrap_or(false)
}

pub fn file_sha256(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)
        .with_context(|| format!("Failed to open package for SHA-256: {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .with_context(|| format!("Failed to read package for SHA-256: {}", path.display()))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(hex_digest(hasher.finalize().as_slice()))
}

fn verify_package_binding(
    label: &str,
    package_path: &Path,
    workspace_root: &Path,
    verification: &PackageVerification,
) -> Result<()> {
    let package_path = fs::canonicalize(package_path)
        .with_context(|| format!("{label} package path cannot be resolved"))?;
    let workspace_dir = fs::canonicalize(&verification.workspace_dir)
        .with_context(|| format!("{label} package workspace cannot be resolved"))?;
    let workspace_root = fs::canonicalize(workspace_root)
        .with_context(|| format!("{label} workspace root cannot be resolved"))?;

    anyhow::ensure!(
        package_path == verification.package_path,
        "{label} package path does not match recorded verification metadata"
    );
    anyhow::ensure!(
        workspace_dir == verification.workspace_dir,
        "{label} package workspace does not match recorded verification metadata"
    );
    anyhow::ensure!(
        workspace_dir.starts_with(&workspace_root),
        "{label} package verification points outside the updater workspace root"
    );
    anyhow::ensure!(
        package_path.starts_with(&workspace_dir),
        "{label} package is outside the verified updater workspace"
    );

    let package_kind = PackageKind::from_path(&package_path);
    anyhow::ensure!(
        package_kind.as_str() == verification.package_kind,
        "{label} package kind does not match recorded verification metadata"
    );

    let current_sha256 = file_sha256(&package_path)?;
    anyhow::ensure!(
        current_sha256 == verification.sha256,
        "{label} package digest does not match recorded verification metadata"
    );
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn package_verification_records_package_digest_and_identity() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let workspace = temp.path().join("workspaces/26.429.20946");
        let dist = workspace.join("dist");
        fs::create_dir_all(&dist)?;
        let package = dist.join("codex-app-26.429.20946-1-x86_64.pkg.tar.zst");
        fs::write(&package, b"package bytes")?;

        let verification = record_built_package(
            &package,
            &workspace,
            "26.429.20946",
            "6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a",
        )?;

        assert_eq!(verification.package_kind, "pacman");
        assert_eq!(verification.package_name, "codex-app");
        assert_eq!(verification.package_version, "26.429.20946-1");
        assert_eq!(verification.package_path, package.canonicalize()?);
        assert_eq!(verification.workspace_dir, workspace.canonicalize()?);
        assert_eq!(
            verification.sha256,
            file_sha256(&package)?,
            "recorded digest should cover exact package bytes"
        );
        Ok(())
    }

    #[test]
    fn package_verification_rejects_package_outside_workspace() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let workspace = temp.path().join("workspaces/26.429.20946");
        let outside = temp
            .path()
            .join("outside/codex-app-26.429.20946-1-x86_64.pkg.tar.zst");
        fs::create_dir_all(&workspace)?;
        fs::create_dir_all(
            outside
                .parent()
                .expect("outside package should have parent"),
        )?;
        fs::write(&outside, b"package bytes")?;

        let error = record_built_package(
            &outside,
            &workspace,
            "26.429.20946",
            "6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a",
        )
        .expect_err("outside package should be rejected");

        assert!(error
            .to_string()
            .contains("Updater-built package is outside its workspace"));
        Ok(())
    }

    #[test]
    fn package_verification_detects_digest_mismatch_before_install() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let workspace = temp.path().join("workspaces/26.429.20946");
        let dist = workspace.join("dist");
        fs::create_dir_all(&dist)?;
        let package = dist.join("codex-app-26.429.20946-1-x86_64.pkg.tar.zst");
        fs::write(&package, b"package bytes")?;
        let verification = record_built_package(
            &package,
            &workspace,
            "26.429.20946",
            "6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a",
        )?;
        fs::write(&package, b"changed package bytes")?;

        let error = expected_package_for_ready_install(
            &package,
            temp.path(),
            Some("26.429.20946"),
            Some("6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a"),
            Some(&verification),
        )
        .expect_err("changed package should fail verification");

        assert!(error
            .to_string()
            .contains("package digest does not match recorded verification metadata"));
        Ok(())
    }
}
