//! Trusted DMG metadata verification for unattended updater rebuilds.

use anyhow::{anyhow, bail, Context, Result};
use reqwest::Url;
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

const TRUSTED_DMG_MANIFEST_RELATIVE_PATH: &str = "updater/trusted-dmg-manifest.json";
const SUPPORTED_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedDmg {
    pub version: String,
    pub sha256: String,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustedDmgManifest {
    schema_version: u32,
    dmgs: Vec<TrustedDmgEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrustedDmgEntry {
    url: String,
    version: String,
    sha256: String,
    #[allow(dead_code)]
    approved_at: Option<String>,
    #[allow(dead_code)]
    notes: Option<String>,
}

pub fn trusted_dmg_manifest_path(builder_bundle_root: &Path) -> PathBuf {
    builder_bundle_root.join(TRUSTED_DMG_MANIFEST_RELATIVE_PATH)
}

pub fn verify_downloaded_dmg_with_manifest(
    manifest_path: &Path,
    dmg_url: &str,
    downloaded_sha256: &str,
) -> Result<VerifiedDmg> {
    let manifest = read_manifest(manifest_path)?;
    let downloaded_sha256 = normalize_sha256(downloaded_sha256)?;

    manifest
        .dmgs
        .iter()
        .find_map(|entry| {
            let trusted_sha256 = normalize_sha256(&entry.sha256).ok()?;
            (entry.url == dmg_url && trusted_sha256 == downloaded_sha256).then(|| VerifiedDmg {
                version: entry.version.clone(),
                sha256: trusted_sha256,
                manifest_path: manifest_path.to_path_buf(),
            })
        })
        .ok_or_else(|| anyhow!("No trusted DMG metadata matched downloaded DMG"))
}

fn read_manifest(path: &Path) -> Result<TrustedDmgManifest> {
    let content = fs::read_to_string(path).with_context(|| {
        format!(
            "Failed to read trusted DMG metadata from {}",
            path.display()
        )
    })?;
    let manifest = serde_json::from_str::<TrustedDmgManifest>(&content).with_context(|| {
        format!(
            "Failed to parse trusted DMG metadata from {}",
            path.display()
        )
    })?;

    if manifest.schema_version != SUPPORTED_SCHEMA_VERSION {
        bail!(
            "Unsupported trusted DMG manifest schema version {}",
            manifest.schema_version
        );
    }

    for entry in &manifest.dmgs {
        validate_manifest_entry(entry)
            .with_context(|| format!("Invalid trusted DMG metadata entry for {}", entry.url))?;
    }

    Ok(manifest)
}

fn validate_manifest_entry(entry: &TrustedDmgEntry) -> Result<()> {
    normalize_sha256(&entry.sha256)?;
    if !valid_dmg_version(&entry.version) {
        bail!("trusted DMG version must have three or four numeric dot-separated segments");
    }
    if entry.url.trim().is_empty() {
        bail!("trusted DMG URL must not be empty");
    }
    if entry.url.trim() != entry.url {
        bail!("trusted DMG URL must not contain leading or trailing whitespace");
    }
    let url = Url::parse(&entry.url).context("trusted DMG URL must be parseable")?;
    if url.scheme() != "https" {
        bail!("trusted DMG URL must use https");
    }
    Ok(())
}

fn normalize_sha256(value: &str) -> Result<String> {
    let value = value.trim();
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("trusted DMG digest must be a 64-character SHA-256 hex digest");
    }
    Ok(value.to_ascii_lowercase())
}

fn valid_dmg_version(version: &str) -> bool {
    let mut count = 0;
    for part in version.split('.') {
        if part.is_empty() || part.parse::<u32>().is_err() {
            return false;
        }
        count += 1;
    }
    matches!(count, 3 | 4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use tempfile::tempdir;

    #[test]
    fn verifies_downloaded_dmg_against_trusted_manifest_entry() -> Result<()> {
        let temp = tempdir()?;
        let manifest_path = trusted_dmg_manifest_path(temp.path());
        fs::create_dir_all(manifest_path.parent().expect("manifest parent"))?;
        fs::write(
            &manifest_path,
            r#"{
  "schema_version": 1,
  "dmgs": [
    {
      "url": "https://persistent.oaistatic.com/codex-app-prod/Codex.dmg",
      "version": "26.513.31313",
      "sha256": "6D440C7133771935C860A5546BCD603F8B9B65B37E9B82BDB0019D4FD0C85B6A",
      "approved_at": "2026-05-18T00:00:00Z",
      "notes": "test fixture"
    }
  ]
}
"#,
        )?;

        let verified = verify_downloaded_dmg_with_manifest(
            &manifest_path,
            "https://persistent.oaistatic.com/codex-app-prod/Codex.dmg",
            "6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a",
        )?;

        assert_eq!(verified.version, "26.513.31313");
        assert_eq!(
            verified.sha256,
            "6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a"
        );
        assert_eq!(verified.manifest_path, manifest_path);
        Ok(())
    }

    #[test]
    fn rejects_downloaded_dmg_without_matching_trusted_metadata() -> Result<()> {
        let temp = tempdir()?;
        let manifest_path = trusted_dmg_manifest_path(temp.path());
        fs::create_dir_all(manifest_path.parent().expect("manifest parent"))?;
        fs::write(
            &manifest_path,
            r#"{
  "schema_version": 1,
  "dmgs": [
    {
      "url": "https://persistent.oaistatic.com/codex-app-prod/Codex.dmg",
      "version": "26.513.31313",
      "sha256": "6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a"
    }
  ]
}
"#,
        )?;

        let error = verify_downloaded_dmg_with_manifest(
            &manifest_path,
            "https://persistent.oaistatic.com/codex-app-prod/Codex.dmg",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .expect_err("digest mismatch should fail");

        assert!(error
            .to_string()
            .contains("No trusted DMG metadata matched downloaded DMG"));
        Ok(())
    }

    #[test]
    fn rejects_downloaded_dmg_with_mismatched_url() -> Result<()> {
        let temp = tempdir()?;
        let manifest_path = trusted_dmg_manifest_path(temp.path());
        fs::create_dir_all(manifest_path.parent().expect("manifest parent"))?;
        fs::write(
            &manifest_path,
            r#"{
  "schema_version": 1,
  "dmgs": [
    {
      "url": "https://persistent.oaistatic.com/codex-app-prod/Codex.dmg",
      "version": "26.513.31313",
      "sha256": "6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a"
    }
  ]
}
"#,
        )?;

        let error = verify_downloaded_dmg_with_manifest(
            &manifest_path,
            "https://example.com/different/Codex.dmg",
            "6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a",
        )
        .expect_err("URL mismatch should fail");

        assert!(error
            .to_string()
            .contains("No trusted DMG metadata matched downloaded DMG"));
        Ok(())
    }

    #[test]
    fn rejects_manifest_urls_with_whitespace() -> Result<()> {
        let temp = tempdir()?;
        let manifest_path = trusted_dmg_manifest_path(temp.path());
        fs::create_dir_all(manifest_path.parent().expect("manifest parent"))?;
        fs::write(
            &manifest_path,
            r#"{
  "schema_version": 1,
  "dmgs": [
    {
      "url": " https://persistent.oaistatic.com/codex-app-prod/Codex.dmg ",
      "version": "26.513.31313",
      "sha256": "6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a"
    }
  ]
}
"#,
        )?;

        let error = verify_downloaded_dmg_with_manifest(
            &manifest_path,
            "https://persistent.oaistatic.com/codex-app-prod/Codex.dmg",
            "6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a",
        )
        .expect_err("manifest URL whitespace should fail validation");

        assert!(error
            .to_string()
            .contains("Invalid trusted DMG metadata entry"));
        Ok(())
    }

    #[test]
    fn rejects_non_https_manifest_urls() -> Result<()> {
        let temp = tempdir()?;
        let manifest_path = trusted_dmg_manifest_path(temp.path());
        fs::create_dir_all(manifest_path.parent().expect("manifest parent"))?;
        fs::write(
            &manifest_path,
            r#"{
  "schema_version": 1,
  "dmgs": [
    {
      "url": "http://persistent.oaistatic.com/codex-app-prod/Codex.dmg",
      "version": "26.513.31313",
      "sha256": "6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a"
    }
  ]
}
"#,
        )?;

        let error = verify_downloaded_dmg_with_manifest(
            &manifest_path,
            "http://persistent.oaistatic.com/codex-app-prod/Codex.dmg",
            "6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a",
        )
        .expect_err("non-https manifest URL should fail validation");

        assert!(error
            .to_string()
            .contains("Invalid trusted DMG metadata entry"));
        Ok(())
    }

    #[test]
    fn reports_missing_manifest_as_metadata_read_failure() -> Result<()> {
        let temp = tempdir()?;

        let error = verify_downloaded_dmg_with_manifest(
            &trusted_dmg_manifest_path(temp.path()),
            "https://persistent.oaistatic.com/codex-app-prod/Codex.dmg",
            "6d440c7133771935c860a5546bcd603f8b9b65b37e9b82bdb0019d4fd0c85b6a",
        )
        .expect_err("missing manifest should fail");

        assert!(error
            .to_string()
            .contains("Failed to read trusted DMG metadata"));
        Ok(())
    }
}
