use serde::{Deserialize, Serialize};

/// Mirrors `config::AppConfig` in the backend.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub input_device: Option<String>,
    pub target_lang: Option<String>,
}

/// Retrieves the current application configuration.
pub async fn get_config() -> Result<AppConfig, String> {
    tauri_sys::core::invoke_result::<AppConfig, String>("get_config", ()).await
}

/// Saves the application configuration.
pub async fn set_config(config: AppConfig) -> Result<(), String> {
    tauri_sys::core::invoke_result::<(), String>("set_config", (config,)).await
}
