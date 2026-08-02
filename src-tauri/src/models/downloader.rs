#![allow(dead_code)]

use anyhow::Result;
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

/// Downloads a URL's content to a destination file path.
///
/// Implemented by `ReqwestDownloader` in production; tests inject a scripted
/// implementation so `ModelManager`'s download logic (skip-if-exists, error
/// propagation, correct destination) is verified without hitting the network.
pub trait ModelDownloader {
    async fn download_to(&self, url: &str, dest: &Path) -> Result<()>;
}

/// Streams a URL's response body directly to disk, without buffering the
/// whole file in memory — model files range from megabytes to gigabytes.
#[derive(Debug, Default, Clone, Copy)]
pub struct ReqwestDownloader;

impl ModelDownloader for ReqwestDownloader {
    async fn download_to(&self, url: &str, dest: &Path) -> Result<()> {
        let response = reqwest::get(url).await?.error_for_status()?;
        let mut stream = response.bytes_stream();

        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Write to a `.part` file and rename on success, so a failed or
        // interrupted download never leaves a corrupt file at `dest`.
        let mut tmp_name = dest.as_os_str().to_owned();
        tmp_name.push(".part");
        let tmp_dest = PathBuf::from(tmp_name);

        let mut file = tokio::fs::File::create(&tmp_dest).await?;
        while let Some(chunk) = stream.next().await {
            file.write_all(&chunk?).await?;
        }
        file.flush().await?;
        drop(file);

        tokio::fs::rename(&tmp_dest, dest).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hits a real, tiny, stable public file to verify the actual streaming
    /// + rename logic against a live server — not part of the default test
    /// run (real network), run manually with `--ignored` when touching this
    /// file. Unit coverage for `ModelManager`'s own logic (skip-if-exists,
    /// error propagation) lives in `manager.rs` against a fake downloader.
    #[tokio::test]
    #[ignore]
    async fn test_real_download_against_live_server() {
        let dest = std::env::temp_dir().join("polyvocal_test_real_download.txt");
        let _ = std::fs::remove_file(&dest);

        ReqwestDownloader
            .download_to(
                "https://raw.githubusercontent.com/octocat/Hello-World/master/README",
                &dest,
            )
            .await
            .unwrap();

        let contents = std::fs::read_to_string(&dest).unwrap();
        assert!(contents.contains("Hello World"));
    }
}
