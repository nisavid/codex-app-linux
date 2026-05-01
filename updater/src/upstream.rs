//! Upstream DMG metadata and download helpers.

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use reqwest::{header, Client, Url};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::{fs::File, io::AsyncWriteExt};

const MAX_DMG_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Selected HTTP metadata used to detect upstream DMG changes.
pub struct RemoteMetadata {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub content_length: Option<u64>,
    pub headers_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Result of downloading the current upstream DMG snapshot.
pub struct DownloadedDmg {
    pub path: PathBuf,
    pub sha256: String,
    pub candidate_version: String,
}

fn validate_dmg_url(dmg_url: &str) -> Result<Url> {
    let url = Url::parse(dmg_url).with_context(|| format!("Invalid DMG URL: {dmg_url}"))?;
    let is_loopback_http = url.scheme() == "http"
        && url
            .host_str()
            .is_some_and(|host| host == "localhost" || host == "127.0.0.1" || host == "::1");
    if url.scheme() != "https" && !is_loopback_http {
        return Err(anyhow!(
            "DMG URL must use https unless it targets loopback http"
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(anyhow!("DMG URL must not contain embedded credentials"));
    }
    Ok(url)
}

fn sanitized_url_for_log(dmg_url: &str) -> String {
    match Url::parse(dmg_url) {
        Ok(mut url) => {
            url.set_query(None);
            url.set_fragment(None);
            url.to_string()
        }
        Err(_) => "<invalid-url>".to_string(),
    }
}

/// Fetches the upstream DMG headers used to detect candidate updates.
pub async fn fetch_remote_metadata(client: &Client, dmg_url: &str) -> Result<RemoteMetadata> {
    let url = validate_dmg_url(dmg_url)?;
    let safe_url = sanitized_url_for_log(dmg_url);
    let response = client
        .head(url)
        .send()
        .await
        .with_context(|| format!("Failed HEAD request for {safe_url}"))?
        .error_for_status()
        .with_context(|| format!("HEAD request for {safe_url} returned an error status"))?;

    let etag = response
        .headers()
        .get(header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let last_modified = response
        .headers()
        .get(header::LAST_MODIFIED)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let content_length = response
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());

    let headers_fingerprint = format!(
        "etag={}|last_modified={}|content_length={}",
        etag.as_deref().unwrap_or(""),
        last_modified.as_deref().unwrap_or(""),
        content_length
            .map(|value| value.to_string())
            .as_deref()
            .unwrap_or("")
    );

    Ok(RemoteMetadata {
        etag,
        last_modified,
        content_length,
        headers_fingerprint,
    })
}

/// Downloads the upstream DMG and derives a package version from its hash.
pub async fn download_dmg(
    client: &Client,
    dmg_url: &str,
    destination_dir: &Path,
    version_timestamp: DateTime<Utc>,
) -> Result<DownloadedDmg> {
    download_dmg_with_limit(
        client,
        dmg_url,
        destination_dir,
        version_timestamp,
        MAX_DMG_BYTES,
    )
    .await
}

async fn download_dmg_with_limit(
    client: &Client,
    dmg_url: &str,
    destination_dir: &Path,
    version_timestamp: DateTime<Utc>,
    max_dmg_bytes: u64,
) -> Result<DownloadedDmg> {
    let url = validate_dmg_url(dmg_url)?;
    let safe_url = sanitized_url_for_log(dmg_url);
    tokio::fs::create_dir_all(destination_dir)
        .await
        .with_context(|| format!("Failed to create {}", destination_dir.display()))?;

    let destination = destination_dir.join("Codex.dmg");
    let partial_destination =
        destination_dir.join(format!(".Codex.dmg.{}.part", std::process::id()));

    let download_result: Result<String> = async {
        let mut file = File::create(&partial_destination)
            .await
            .with_context(|| format!("Failed to create {}", partial_destination.display()))?;

        let response = client
            .get(url)
            .send()
            .await
            .with_context(|| format!("Failed GET request for {safe_url}"))?
            .error_for_status()
            .with_context(|| format!("GET request for {safe_url} returned an error status"))?;

        let content_length = response
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .or_else(|| response.content_length());
        if let Some(content_length) = content_length {
            if content_length > max_dmg_bytes {
                return Err(anyhow!(
                    "DMG response for {safe_url} is too large: {content_length} bytes exceeds {max_dmg_bytes}"
                ));
            }
        }

        let mut downloaded_bytes = 0_u64;
        let mut hasher = Sha256::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.with_context(|| format!("Failed downloading {safe_url}"))?;
            downloaded_bytes = downloaded_bytes
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| anyhow!("DMG download byte count overflowed"))?;
            if downloaded_bytes > max_dmg_bytes {
                return Err(anyhow!(
                    "DMG response for {safe_url} exceeded {max_dmg_bytes} bytes while downloading"
                ));
            }
            file.write_all(&chunk)
                .await
                .with_context(|| format!("Failed writing {}", partial_destination.display()))?;
            hasher.update(&chunk);
        }

        file.flush()
            .await
            .with_context(|| format!("Failed flushing {}", partial_destination.display()))?;
        drop(file);

        tokio::fs::rename(&partial_destination, &destination)
            .await
            .with_context(|| {
                format!(
                    "Failed moving {} to {}",
                    partial_destination.display(),
                    destination.display()
                )
            })?;

        Ok(hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>())
    }
    .await;

    let sha256 = match download_result {
        Ok(sha256) => sha256,
        Err(error) => {
            let _ = tokio::fs::remove_file(&partial_destination).await;
            return Err(error);
        }
    };
    let candidate_version = derive_candidate_version(&sha256, version_timestamp)?;

    Ok(DownloadedDmg {
        path: destination,
        sha256,
        candidate_version,
    })
}

/// Derives a local package version from the DMG hash and download timestamp.
pub fn derive_candidate_version(sha256: &str, timestamp: DateTime<Utc>) -> Result<String> {
    let short_hash = sha256
        .get(0..8)
        .ok_or_else(|| anyhow!("sha256 is too short to derive candidate version"))?;
    Ok(format!(
        "{}+{}",
        timestamp.format("%Y.%m.%d.%H%M%S"),
        short_hash
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use chrono::TimeZone;
    use tempfile::tempdir;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[tokio::test]
    async fn fetches_remote_metadata_from_head() -> Result<()> {
        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .and(path("/Codex.dmg"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("ETag", "\"abc\"")
                    .insert_header("Last-Modified", "Tue, 25 Mar 2026 00:00:00 GMT")
                    .insert_header("Content-Length", "42"),
            )
            .mount(&server)
            .await;

        let client = Client::builder().build()?;
        let metadata =
            fetch_remote_metadata(&client, &format!("{}/Codex.dmg", server.uri())).await?;
        assert_eq!(metadata.etag.as_deref(), Some("\"abc\""));
        assert_eq!(
            metadata.last_modified.as_deref(),
            Some("Tue, 25 Mar 2026 00:00:00 GMT")
        );
        assert_eq!(metadata.content_length, Some(42));
        assert!(metadata.headers_fingerprint.contains("etag=\"abc\""));
        Ok(())
    }

    #[tokio::test]
    async fn downloads_dmg_and_hashes_contents() -> Result<()> {
        let server = MockServer::start().await;
        let body = b"codex-dmg-test-payload";
        Mock::given(method("GET"))
            .and(path("/Codex.dmg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.to_vec()))
            .mount(&server)
            .await;

        let client = Client::builder().build()?;
        let temp = tempdir()?;
        let downloaded = download_dmg(
            &client,
            &format!("{}/Codex.dmg", server.uri()),
            temp.path(),
            Utc.with_ymd_and_hms(2026, 3, 24, 12, 0, 0).unwrap(),
        )
        .await?;

        assert_eq!(downloaded.path, temp.path().join("Codex.dmg"));
        assert_eq!(
            downloaded.sha256,
            "678cd508ffe0071e217020a7a4eecbebe25362c022ac78c13a5ae87b7a3a0c92"
        );
        assert_eq!(downloaded.candidate_version, "2026.03.24.120000+678cd508");
        Ok(())
    }

    #[tokio::test]
    async fn rejects_dmg_that_exceeds_content_length_limit() -> Result<()> {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/Codex.dmg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'x'; 9]))
            .mount(&server)
            .await;

        let client = Client::builder().build()?;
        let temp = tempdir()?;
        let error = download_dmg_with_limit(
            &client,
            &format!("{}/Codex.dmg", server.uri()),
            temp.path(),
            Utc.with_ymd_and_hms(2026, 3, 24, 12, 0, 0).unwrap(),
            8,
        )
        .await
        .expect_err("oversized DMG should fail");

        assert!(error.to_string().contains("too large"));
        assert!(!temp.path().join("Codex.dmg").exists());
        Ok(())
    }

    #[test]
    fn sanitizes_dmg_urls_for_logs() {
        assert_eq!(
            sanitized_url_for_log("https://example.com/Codex.dmg?token=secret#frag"),
            "https://example.com/Codex.dmg"
        );
    }

    #[test]
    fn rejects_non_https_non_loopback_dmg_urls() {
        assert!(validate_dmg_url("http://example.com/Codex.dmg").is_err());
        assert!(validate_dmg_url("https://user:pass@example.com/Codex.dmg").is_err());
        assert!(validate_dmg_url("http://127.0.0.1/Codex.dmg").is_ok());
    }

    #[test]
    fn derive_candidate_version_rejects_short_hashes() {
        let error = derive_candidate_version("short", Utc::now()).expect_err("hash should fail");
        assert!(error.to_string().contains("sha256 is too short"));
    }
}
