use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TranslateArgs<'a> {
    session_id: &'a str,
    target_lang: &'a str,
}

/// Mirrors `commands::translation::LanguagePairInfo` — a command *return
/// value*, serialized as-is by the backend's own (unrenamed) `Serialize`
/// impl, same reasoning as `ModelInfo` above.
#[derive(Deserialize, Clone)]
pub struct LanguagePairInfo {
    pub language: String,
    pub size_mb: u32,
    pub downloaded: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadTranslationModelArgs {
    language: String,
}

/// Translates a session's transcript into the specified target language.
/// Returns the translated text.
pub async fn translate_text(session_id: &str, target_lang: &str) -> Result<String, String> {
    let args = TranslateArgs {
        session_id,
        target_lang,
    };
    tauri_sys::core::invoke_result::<String, String>("translate_text", args).await
}

/// Lists all available translation language pairs and their download status.
pub async fn list_translation_models() -> Result<Vec<LanguagePairInfo>, String> {
    tauri_sys::core::invoke_result::<Vec<LanguagePairInfo>, String>("list_translation_models", ())
        .await
}

/// Downloads a translation model for the specified language.
pub async fn download_translation_model(language: &str) -> Result<(), String> {
    let args = DownloadTranslationModelArgs {
        language: language.to_string(),
    };
    tauri_sys::core::invoke_result::<(), String>("download_translation_model", args).await
}
