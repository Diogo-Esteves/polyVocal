#![allow(dead_code)]

use super::registry::{ModelInfo, ModelSize};
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

    /// List all known models with their local availability.
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

    /// Download a model (streaming, with progress).
    /// TODO: emit Tauri events for download progress.
    pub async fn download(&self, size: &ModelSize) -> Result<()> {
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

        // TODO: streaming download with reqwest, emitting progress events
        // to the Tauri frontend via app_handle.emit_all("model:progress", ...)

        Ok(())
    }
}
