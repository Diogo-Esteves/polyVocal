use crate::models::downloader::{ModelDownloader, ReqwestDownloader};
use crate::models::manager::ModelManager;
use crate::models::registry::{ModelInfo, ModelSize};
use tauri::{AppHandle, Manager};

fn model_manager(app: &AppHandle) -> Result<ModelManager, String> {
    let models_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("models");
    Ok(ModelManager::new(models_dir))
}

#[tauri::command]
pub async fn list_models(app: AppHandle) -> Result<Vec<ModelInfo>, String> {
    Ok(model_manager(&app)?.list())
}

#[tauri::command]
pub async fn download_model(app: AppHandle, size: ModelSize) -> Result<(), String> {
    model_manager(&app)?
        .download(&size, &ReqwestDownloader)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_active_model(app: AppHandle, size: ModelSize) -> Result<(), String> {
    model_manager(&app)?
        .set_active(&size)
        .map_err(|e| e.to_string())
}

/// Downloads and activates the `tiny` Whisper model, unless a model is
/// already active — so a fresh install can transcribe immediately, without
/// requiring a trip to Settings first. A no-op on every later launch, once
/// any model (not necessarily `tiny`) has been activated.
async fn ensure_default_model_with<D: ModelDownloader>(
    manager: &ModelManager,
    downloader: &D,
) -> Result<(), String> {
    if manager.active_model_path().is_some() {
        return Ok(());
    }

    manager
        .download(&ModelSize::Tiny, downloader)
        .await
        .map_err(|e| e.to_string())?;
    manager
        .set_active(&ModelSize::Tiny)
        .map_err(|e| e.to_string())
}

pub async fn ensure_default_model(app: &AppHandle) -> Result<(), String> {
    let manager = model_manager(app)?;
    ensure_default_model_with(&manager, &ReqwestDownloader).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result as AnyhowResult;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

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

        fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
    }

    impl ModelDownloader for FakeDownloader {
        async fn download_to(&self, url: &str, dest: &Path) -> AnyhowResult<()> {
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
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn ensure_default_model_downloads_and_activates_tiny_when_none_active() {
        let dir = temp_models_dir("polyvocal_test_ensure_default_fresh");
        let manager = ModelManager::new(dir);
        let downloader = FakeDownloader::success(b"fake tiny model");

        ensure_default_model_with(&manager, &downloader)
            .await
            .unwrap();

        assert_eq!(downloader.call_count(), 1);
        assert!(manager.is_active(&ModelSize::Tiny));
    }

    #[tokio::test]
    async fn ensure_default_model_is_noop_when_a_model_is_already_active() {
        let dir = temp_models_dir("polyvocal_test_ensure_default_existing");
        let manager = ModelManager::new(dir);
        let downloader = FakeDownloader::success(b"fake base model");

        // Pre-seed an already-active (non-tiny) model.
        manager
            .download(&ModelSize::Base, &downloader)
            .await
            .unwrap();
        manager.set_active(&ModelSize::Base).unwrap();

        ensure_default_model_with(&manager, &downloader)
            .await
            .unwrap();

        // No further downloads, and the pre-existing choice is left alone.
        assert_eq!(downloader.call_count(), 1);
        assert!(manager.is_active(&ModelSize::Base));
    }
}
