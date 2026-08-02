use crate::models::downloader::ReqwestDownloader;
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
