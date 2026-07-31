#[tauri::command]
pub async fn translate_text(_session_id: String, _target_lang: String) -> Result<String, String> {
    // TODO: fetch transcript from DB, call TranslationClient, persist translation,
    //       return translated text
    Ok(String::new())
}
