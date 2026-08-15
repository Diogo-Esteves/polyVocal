use anyhow::Result;
use futures_util::StreamExt;
use std::future::Future;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

/// Downloads a URL's content to a destination file path, verifying it
/// matches `expected_sha256` before it's left on disk for anything else to
/// read — see issue #54. Checksumming lives at this trait boundary (not a
/// separate step in `ModelManager`) so it only ever runs against bytes that
/// actually came off the network; `ModelManager`'s tests inject a scripted
/// fake that writes arbitrary fixture bytes and has no reason to verify
/// them against a real production hash.
///
/// Implemented by `ReqwestDownloader` in production; tests inject a scripted
/// implementation so `ModelManager`'s download logic (skip-if-exists, error
/// propagation, correct destination) is verified without hitting the network.
///
/// Uses return-position `impl Future + Send` rather than `async fn` because
/// this trait is genuinely public API (used from `#[tauri::command]`s, which
/// require `Send` futures) — plain `async fn` in a public trait can't
/// express that bound.
pub trait ModelDownloader {
    fn download_to(
        &self,
        url: &str,
        dest: &Path,
        expected_sha256: &str,
    ) -> impl Future<Output = Result<()>> + Send;
}

/// Returns an error if `bytes`'s SHA256 doesn't match `expected_sha256`.
/// Pure and network-free so it's unit-testable directly, independent of
/// `ReqwestDownloader`'s real HTTP fetch.
fn verify_sha256(bytes: &[u8], expected_sha256: &str) -> Result<()> {
    use sha2::{Digest, Sha256};
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected_sha256 {
        anyhow::bail!("checksum mismatch: expected {expected_sha256}, got {actual}");
    }
    Ok(())
}

/// Streams a URL's response body directly to disk, without buffering the
/// whole file in memory — model files range from megabytes to gigabytes.
#[derive(Debug, Default, Clone, Copy)]
pub struct ReqwestDownloader;

impl ModelDownloader for ReqwestDownloader {
    async fn download_to(&self, url: &str, dest: &Path, expected_sha256: &str) -> Result<()> {
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

        let bytes = tokio::fs::read(&tmp_dest).await?;
        if let Err(e) = verify_sha256(&bytes, expected_sha256) {
            tokio::fs::remove_file(&tmp_dest).await.ok();
            return Err(e);
        }

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
                "03ba204e50d126e4674c005e04d82e84c21366780af1f43bd54a37816b6ab340",
            )
            .await
            .unwrap();

        let contents = std::fs::read_to_string(&dest).unwrap();
        assert!(contents.contains("Hello World"));
    }

    #[test]
    fn test_verify_sha256_matches() {
        let bytes = b"test content for verification";
        let expected = format!("{:x}", {
            use sha2::{Digest, Sha256};
            Sha256::digest(bytes)
        });
        assert!(verify_sha256(bytes, &expected).is_ok());
    }

    #[test]
    fn test_verify_sha256_mismatch() {
        let bytes = b"test content";
        let wrong = "0".repeat(64);
        assert!(verify_sha256(bytes, &wrong).is_err());
    }
}
