use tauri::{AppHandle, Manager};

fn config_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("config.toml"))
}

#[tauri::command]
pub async fn get_config(app: AppHandle) -> Result<crate::config::AppConfig, String> {
    Ok(crate::config::load(&config_path(&app)?))
}

#[tauri::command]
pub async fn set_config(app: AppHandle, config: crate::config::AppConfig) -> Result<(), String> {
    crate::config::save(&config_path(&app)?, &config).map_err(|e| e.to_string())
}
