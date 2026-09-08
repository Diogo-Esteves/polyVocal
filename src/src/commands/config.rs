use serde::{Deserialize, Serialize};

/// Mirrors `config::AppConfig` in the backend.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub input_device: Option<String>,
    pub target_lang: Option<String>,
    pub streaming_partials_enabled: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SetConfigArgs {
    config: AppConfig,
}

/// Retrieves the current application configuration.
pub async fn get_config() -> Result<AppConfig, String> {
    tauri_sys::core::invoke_result::<AppConfig, String>("get_config", ()).await
}

/// Saves the application configuration.
pub async fn set_config(config: AppConfig) -> Result<(), String> {
    let args = SetConfigArgs { config };
    tauri_sys::core::invoke_result::<(), String>("set_config", args).await
}
