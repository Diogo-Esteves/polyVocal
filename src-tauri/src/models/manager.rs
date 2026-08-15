use super::downloader::ModelDownloader;
use super::registry::{ModelInfo, ModelSize, TranslationModel, VadModel, TRANSLATION_MODEL_FILES};
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

        info!("Downloading {} ({} MB)", size.filename(), size.size_mb());
        downloader
            .download_to(size.download_url(), &dest, size.sha256())
            .await?;
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
            info!("Downloading VAD model {}", model.filename());
            downloader
                .download_to(model.download_url(), &dest, model.sha256())
                .await?;
        }
        Ok(dest)
    }

    /// Directory a translation model's files live (or would live) in.
    pub fn translation_model_dir(&self, model: &TranslationModel) -> PathBuf {
        self.models_dir.join("translation").join(model.dir_name())
    }

    /// Whether every file `ensure_translation_model` would fetch for `model`
    /// is already on disk — a read-only check Settings uses to show
    /// ready/download state without triggering a download.
    pub fn is_translation_model_downloaded(&self, model: &TranslationModel) -> bool {
        let dir = self.translation_model_dir(model);
        TRANSLATION_MODEL_FILES
            .iter()
            .all(|file| dir.join(file).exists())
    }

    /// Download all of a translation model's files, unless already present,
    /// returning the directory containing them. Each file is checked
    /// independently so an interrupted download only re-fetches what's
    /// missing, not the whole model.
    pub async fn ensure_translation_model<D: ModelDownloader>(
        &self,
        model: &TranslationModel,
        downloader: &D,
    ) -> Result<PathBuf> {
        let dir = self.translation_model_dir(model);
        std::fs::create_dir_all(&dir)?;
        for file in TRANSLATION_MODEL_FILES {
            let dest = dir.join(file);
            if dest.exists() {
                continue;
            }
            info!(
                "Downloading translation model file {}/{}",
                model.dir_name(),
                file
            );
            downloader
                .download_to(&model.download_url(file), &dest, model.sha256(file))
                .await?;
        }
        Ok(dir)
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
        async fn download_to(&self, url: &str, dest: &Path, _expected_sha256: &str) -> Result<()> {
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

    #[tokio::test]
    async fn test_ensure_translation_model_downloads_all_files() {
        let dir = temp_models_dir("polyvocal_test_translation_download");
        let model_dir = dir
            .join("translation")
            .join(TranslationModel::EnEs.dir_name());
        let _ = std::fs::remove_dir_all(&model_dir);

        let manager = ModelManager::new(dir.clone());
        let downloader = FakeDownloader::success(b"fake weights");

        let path = manager
            .ensure_translation_model(&TranslationModel::EnEs, &downloader)
            .await
            .unwrap();

        assert_eq!(path, model_dir);
        assert_eq!(downloader.call_count(), TRANSLATION_MODEL_FILES.len());
        for file in TRANSLATION_MODEL_FILES {
            assert_eq!(
                std::fs::read(model_dir.join(file)).unwrap(),
                b"fake weights"
            );
        }
    }

    #[tokio::test]
    async fn test_ensure_translation_model_only_downloads_missing_files() {
        let dir = temp_models_dir("polyvocal_test_translation_partial");
        let model_dir = dir
            .join("translation")
            .join(TranslationModel::EnEs.dir_name());
        let _ = std::fs::remove_dir_all(&model_dir);
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("config.json"), b"already here").unwrap();

        let manager = ModelManager::new(dir.clone());
        let downloader = FakeDownloader::success(b"freshly downloaded");

        manager
            .ensure_translation_model(&TranslationModel::EnEs, &downloader)
            .await
            .unwrap();

        assert_eq!(downloader.call_count(), TRANSLATION_MODEL_FILES.len() - 1);
        assert_eq!(
            std::fs::read(model_dir.join("config.json")).unwrap(),
            b"already here"
        );
        assert_eq!(
            std::fs::read(model_dir.join("vocab.json")).unwrap(),
            b"freshly downloaded"
        );
    }

    #[test]
    fn test_is_translation_model_downloaded_false_when_dir_missing() {
        let dir = temp_models_dir("polyvocal_test_translation_downloaded_missing");
        let model_dir = dir
            .join("translation")
            .join(TranslationModel::EnEs.dir_name());
        let _ = std::fs::remove_dir_all(&model_dir);

        let manager = ModelManager::new(dir.clone());

        assert!(!manager.is_translation_model_downloaded(&TranslationModel::EnEs));
    }

    #[test]
    fn test_is_translation_model_downloaded_false_when_some_files_missing() {
        let dir = temp_models_dir("polyvocal_test_translation_downloaded_partial");
        let model_dir = dir
            .join("translation")
            .join(TranslationModel::EnEs.dir_name());
        let _ = std::fs::remove_dir_all(&model_dir);
        std::fs::create_dir_all(&model_dir).unwrap();
        for file in TRANSLATION_MODEL_FILES
            .iter()
            .take(TRANSLATION_MODEL_FILES.len() - 1)
        {
            std::fs::write(model_dir.join(file), b"present").unwrap();
        }

        let manager = ModelManager::new(dir.clone());

        assert!(!manager.is_translation_model_downloaded(&TranslationModel::EnEs));
    }

    #[test]
    fn test_is_translation_model_downloaded_true_when_all_files_present() {
        let dir = temp_models_dir("polyvocal_test_translation_downloaded_complete");
        let model_dir = dir
            .join("translation")
            .join(TranslationModel::EnEs.dir_name());
        let _ = std::fs::remove_dir_all(&model_dir);
        std::fs::create_dir_all(&model_dir).unwrap();
        for file in TRANSLATION_MODEL_FILES {
            std::fs::write(model_dir.join(file), b"present").unwrap();
        }

        let manager = ModelManager::new(dir.clone());

        assert!(manager.is_translation_model_downloaded(&TranslationModel::EnEs));
    }
}
