#![allow(dead_code)]

use super::downloader::ModelDownloader;
use super::registry::{ModelInfo, ModelSize, VadModel};
use anyhow::Result;
use std::path::PathBuf;
use tracing::info;

pub struct ModelManager {
    models_dir: PathBuf,
}

impl ModelManager {
    pub fn new(models_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&models_dir).ok();
        Self { models_dir }
    }

    /// List all known Whisper models with their local availability.
    pub fn list(&self) -> Vec<ModelInfo> {
        let sizes = [
            ModelSize::Tiny,
            ModelSize::Base,
            ModelSize::Small,
            ModelSize::Medium,
        ];

        sizes
            .into_iter()
            .map(|size| {
                let path = self.models_dir.join(size.filename());
                ModelInfo {
                    downloaded: path.exists(),
                    is_active: self.is_active(&size),
                    size,
                }
            })
            .collect()
    }

    /// Check if a model is the currently active one.
    pub fn is_active(&self, size: &ModelSize) -> bool {
        self.active_model_path()
            .map(|p| p.file_name() == self.models_dir.join(size.filename()).file_name())
            .unwrap_or(false)
    }

    /// Path to the currently active model, if any.
    pub fn active_model_path(&self) -> Option<PathBuf> {
        let marker = self.models_dir.join(".active");
        std::fs::read_to_string(marker)
            .ok()
            .map(|s| self.models_dir.join(s.trim()))
    }

    /// Set the active model.
    pub fn set_active(&self, size: &ModelSize) -> Result<()> {
        let path = self.models_dir.join(size.filename());
        anyhow::ensure!(path.exists(), "Model not downloaded: {:?}", size);
        std::fs::write(self.models_dir.join(".active"), size.filename())?;
        info!("Active model set to: {}", size.filename());
        Ok(())
    }

    /// Download a Whisper model, unless it's already present.
    pub async fn download<D: ModelDownloader>(
        &self,
        size: &ModelSize,
        downloader: &D,
    ) -> Result<()> {
        let dest = self.models_dir.join(size.filename());
        if dest.exists() {
            info!("Model already downloaded: {}", size.filename());
            return Ok(());
        }

        info!(
            "Downloading {} ({} MB) → {}",
            size.filename(),
            size.size_mb(),
            dest.display()
        );
        downloader.download_to(size.download_url(), &dest).await?;
        Ok(())
    }

    /// Path a VAD model lives (or would live) at.
    pub fn vad_model_path(&self, model: &VadModel) -> PathBuf {
        self.models_dir.join(model.filename())
    }

    /// Download a VAD model, unless it's already present, returning its path.
    pub async fn ensure_vad_model<D: ModelDownloader>(
        &self,
        model: &VadModel,
        downloader: &D,
    ) -> Result<PathBuf> {
        let dest = self.vad_model_path(model);
        if !dest.exists() {
            info!(
                "Downloading VAD model {} → {}",
                model.filename(),
                dest.display()
            );
            downloader.download_to(model.download_url(), &dest).await?;
        }
        Ok(dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Mutex;

    /// Records the (url, dest) it was called with and either writes fixed
    /// bytes to `dest` or returns a scripted error — no real network I/O.
    struct FakeDownloader {
        result: Result<Vec<u8>, String>,
        calls: Mutex<Vec<(String, PathBuf)>>,
    }

    impl FakeDownloader {
        fn success(bytes: &[u8]) -> Self {
            Self {
                result: Ok(bytes.to_vec()),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn failure(message: &str) -> Self {
            Self {
                result: Err(message.to_string()),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
    }

    impl ModelDownloader for FakeDownloader {
        async fn download_to(&self, url: &str, dest: &Path) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push((url.to_string(), dest.to_path_buf()));

            match &self.result {
                Ok(bytes) => {
                    std::fs::write(dest, bytes)?;
                    Ok(())
                }
                Err(msg) => Err(anyhow::anyhow!("{msg}")),
            }
        }
    }

    fn temp_models_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn test_download_writes_model_file() {
        let dir = temp_models_dir("polyvocal_test_download_writes");
        let dest = dir.join(ModelSize::Tiny.filename());
        let _ = std::fs::remove_file(&dest); // guarantee a fresh, not-yet-downloaded state

        let manager = ModelManager::new(dir.clone());
        let downloader = FakeDownloader::success(b"fake model bytes");

        manager
            .download(&ModelSize::Tiny, &downloader)
            .await
            .unwrap();

        assert_eq!(std::fs::read(&dest).unwrap(), b"fake model bytes");
        assert_eq!(downloader.call_count(), 1);
    }

    #[tokio::test]
    async fn test_download_skips_if_already_present() {
        let dir = temp_models_dir("polyvocal_test_download_skips");
        let dest = dir.join(ModelSize::Tiny.filename());
        std::fs::write(&dest, b"already here").unwrap();

        let manager = ModelManager::new(dir.clone());
        let downloader = FakeDownloader::success(b"should not be written");

        manager
            .download(&ModelSize::Tiny, &downloader)
            .await
            .unwrap();

        assert_eq!(downloader.call_count(), 0);
        assert_eq!(std::fs::read(&dest).unwrap(), b"already here");
    }

    #[tokio::test]
    async fn test_download_propagates_downloader_error() {
        let dir = temp_models_dir("polyvocal_test_download_error");
        let dest = dir.join(ModelSize::Tiny.filename());
        let _ = std::fs::remove_file(&dest);

        let manager = ModelManager::new(dir.clone());
        let downloader = FakeDownloader::failure("network unreachable");

        let result = manager.download(&ModelSize::Tiny, &downloader).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("network unreachable"));
        assert!(!dest.exists());
    }

    #[tokio::test]
    async fn test_ensure_vad_model_downloads_when_missing() {
        let dir = temp_models_dir("polyvocal_test_vad_download");
        let dest = dir.join(VadModel::Silero.filename());
        let _ = std::fs::remove_file(&dest);

        let manager = ModelManager::new(dir.clone());
        let downloader = FakeDownloader::success(b"fake onnx bytes");

        let path = manager
            .ensure_vad_model(&VadModel::Silero, &downloader)
            .await
            .unwrap();

        assert_eq!(path, dest);
        assert_eq!(std::fs::read(&path).unwrap(), b"fake onnx bytes");
        assert_eq!(downloader.call_count(), 1);
    }

    #[tokio::test]
    async fn test_ensure_vad_model_skips_if_present() {
        let dir = temp_models_dir("polyvocal_test_vad_skip");
        let dest = dir.join(VadModel::Silero.filename());
        std::fs::write(&dest, b"already here").unwrap();

        let manager = ModelManager::new(dir.clone());
        let downloader = FakeDownloader::success(b"should not overwrite");

        manager
            .ensure_vad_model(&VadModel::Silero, &downloader)
            .await
            .unwrap();

        assert_eq!(downloader.call_count(), 0);
        assert_eq!(std::fs::read(&dest).unwrap(), b"already here");
    }
}
