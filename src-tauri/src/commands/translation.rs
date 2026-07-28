use tauri::State;

#[tauri::command]
pub async fn translate_text(
    session_id: String,
    target_lang: String,
) -> Result<String, String> {
    // TODO: fetch transcript from DB, call TranslationClient, persist translation,
    //       return translated text
    Ok(String::new())
}
