use crate::models::registry::{ModelInfo, ModelSize};

#[tauri::command]
pub async fn list_models() -> Result<Vec<ModelInfo>, String> {
    // TODO: resolve ModelManager from Tauri state, call list()
    Ok(vec![])
}

#[tauri::command]
pub async fn download_model(_size: ModelSize) -> Result<(), String> {
    // TODO: resolve ModelManager from Tauri state, call download()
    Ok(())
}

#[tauri::command]
pub async fn set_active_model(_size: ModelSize) -> Result<(), String> {
    // TODO: resolve ModelManager from Tauri state, call set_active()
    Ok(())
}
