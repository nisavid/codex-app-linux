//! Upstream DMG metadata and download helpers.

use anyhow::{ensure, Context, Result};
use futures_util::StreamExt;
use reqwest::{header, Client, Url};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};

const MAX_DMG_BYTES: u64 = 2 * 1024 * 1024 * 1024;

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
}

/// Fetches the upstream DMG headers used to detect candidate updates.
pub async fn fetch_remote_metadata(client: &Client, dmg_url: &str) -> Result<RemoteMetadata> {
    let url = parse_dmg_url(dmg_url)?;
    let safe_url = safe_url_for_log(url.as_str());
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

/// Downloads the upstream DMG and hashes its contents.
pub async fn download_dmg(
    client: &Client,
    dmg_url: &str,
    destination_dir: &Path,
) -> Result<DownloadedDmg> {
    download_dmg_with_max_bytes(client, dmg_url, destination_dir, MAX_DMG_BYTES).await
}

async fn download_dmg_with_max_bytes(
    client: &Client,
    dmg_url: &str,
    destination_dir: &Path,
    max_bytes: u64,
) -> Result<DownloadedDmg> {
    let url = parse_dmg_url(dmg_url)?;
    let safe_url = safe_url_for_log(url.as_str());

    tokio::fs::create_dir_all(destination_dir)
        .await
        .with_context(|| format!("Failed to create {}", destination_dir.display()))?;

    let destination = destination_dir.join("Codex.dmg");
    let temp_destination = destination_dir.join(".Codex.dmg.tmp");
    let _ = fs::remove_file(&temp_destination).await;

    let result: Result<DownloadedDmg> = async {
        let response = client
            .get(url)
            .send()
            .await
            .with_context(|| format!("Failed GET request for {safe_url}"))?
            .error_for_status()
            .with_context(|| format!("GET request for {safe_url} returned an error status"))?;
        if let Some(content_length) = response.content_length() {
            ensure!(
                content_length <= max_bytes,
                "DMG download size {content_length} exceeds maximum {max_bytes} bytes"
            );
        }

        let mut file = File::create(&temp_destination)
            .await
            .with_context(|| format!("Failed to create {}", temp_destination.display()))?;
        let mut hasher = Sha256::new();
        let mut stream = response.bytes_stream();
        let mut downloaded_bytes = 0_u64;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.with_context(|| format!("Failed downloading {safe_url}"))?;
            downloaded_bytes += chunk.len() as u64;
            if downloaded_bytes > max_bytes {
                ensure!(
                    downloaded_bytes <= max_bytes,
                    "DMG download size exceeds maximum {max_bytes} bytes"
                );
            }
            file.write_all(&chunk)
                .await
                .with_context(|| format!("Failed writing {}", temp_destination.display()))?;
            hasher.update(&chunk);
        }

        file.flush()
            .await
            .with_context(|| format!("Failed flushing {}", temp_destination.display()))?;
        drop(file);

        let sha256 = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();

        fs::rename(&temp_destination, &destination)
            .await
            .with_context(|| {
                format!(
                    "Failed moving {} to {}",
                    temp_destination.display(),
                    destination.display()
                )
            })?;

        Ok(DownloadedDmg {
            path: destination,
            sha256,
        })
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&temp_destination).await;
    }
    result
}

fn parse_dmg_url(dmg_url: &str) -> Result<Url> {
    let url = Url::parse(dmg_url).context("Failed to parse DMG URL")?;
    let is_loopback_host = matches!(
        url.host_str(),
        Some("localhost") | Some("127.0.0.1") | Some("::1")
    );
    ensure!(url.host_str().is_some(), "DMG URL must include a host");
    ensure!(
        url.scheme() == "https" || (url.scheme() == "http" && is_loopback_host),
        "DMG URL must use https unless it targets localhost/127.0.0.1/::1 over http"
    );
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "DMG URL must not include userinfo"
    );
    Ok(url)
}

fn safe_url_for_log(dmg_url: &str) -> String {
    let Ok(mut url) = Url::parse(dmg_url) else {
        return "<invalid URL>".to_string();
    };
    let had_fragment = url.fragment().is_some();
    url.set_fragment(None);
    if url.query().is_some() {
        url.set_query(None);
        let mut safe_url = url.to_string();
        safe_url.push_str("?<redacted>");
        if had_fragment {
            safe_url.push_str("#<redacted>");
        }
        return safe_url;
    }
    let mut safe_url = url.to_string();
    if had_fragment {
        safe_url.push_str("#<redacted>");
    }
    safe_url
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
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
    async fn rejects_dmg_url_with_userinfo_before_request() -> Result<()> {
        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .and(path("/Codex.dmg"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = Client::builder().build()?;
        let error = fetch_remote_metadata(
            &client,
            &server
                .uri()
                .replacen("://", "://user:secret@", 1)
                .replace("127.0.0.1", "localhost"),
        )
        .await
        .expect_err("URL userinfo should be rejected");

        assert!(error
            .to_string()
            .contains("DMG URL must not include userinfo"));
        assert!(!error.to_string().contains("secret"));
        Ok(())
    }

    #[tokio::test]
    async fn rejects_non_https_non_loopback_dmg_url_before_request() -> Result<()> {
        let client = Client::builder().build()?;
        let error = fetch_remote_metadata(&client, "http://example.com/Codex.dmg")
            .await
            .expect_err("non-HTTPS URL should be rejected");

        assert!(error
            .to_string()
            .contains("DMG URL must use https unless it targets localhost"));
        Ok(())
    }

    #[test]
    fn redacts_query_and_fragment_values_for_url_logging() {
        assert_eq!(
            safe_url_for_log("https://example.com/Codex.dmg?token=secret&build=123#nonce"),
            "https://example.com/Codex.dmg?<redacted>#<redacted>"
        );
        assert_eq!(
            safe_url_for_log("https://example.com/Codex.dmg#nonce"),
            "https://example.com/Codex.dmg#<redacted>"
        );
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
        let downloaded =
            download_dmg(&client, &format!("{}/Codex.dmg", server.uri()), temp.path()).await?;

        assert_eq!(downloaded.path, temp.path().join("Codex.dmg"));
        assert_eq!(
            downloaded.sha256,
            "678cd508ffe0071e217020a7a4eecbebe25362c022ac78c13a5ae87b7a3a0c92"
        );
        Ok(())
    }

    #[tokio::test]
    async fn rejects_dmg_when_content_length_exceeds_limit() -> Result<()> {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/Codex.dmg"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("Content-Length", "9")
                    .set_body_bytes(b"too large".to_vec()),
            )
            .mount(&server)
            .await;

        let client = Client::builder().build()?;
        let temp = tempdir()?;
        let error = download_dmg_with_max_bytes(
            &client,
            &format!("{}/Codex.dmg", server.uri()),
            temp.path(),
            8,
        )
        .await
        .expect_err("oversized Content-Length should be rejected");

        assert!(error.to_string().contains("exceeds maximum"));
        assert!(!temp.path().join("Codex.dmg").exists());
        Ok(())
    }
}
