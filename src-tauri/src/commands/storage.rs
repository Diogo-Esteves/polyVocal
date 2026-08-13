use crate::storage::models::Session;
use crate::storage::repository::SessionRepository;
use sqlx::SqlitePool;
use tauri::State;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn list_sessions(
    pool: State<'_, SqlitePool>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<Session>, String> {
    let repository = SessionRepository::new(pool.inner().clone());
    repository
        .list(limit.unwrap_or(50), offset.unwrap_or(0))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_session(
    pool: State<'_, SqlitePool>,
    id: String,
) -> Result<Option<Session>, String> {
    let repository = SessionRepository::new(pool.inner().clone());
    repository.get(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_session(pool: State<'_, SqlitePool>, id: String) -> Result<(), String> {
    let repository = SessionRepository::new(pool.inner().clone());
    repository.delete(&id).await.map_err(|e| e.to_string())
}

/// Renders a session as plain text for the "export as TXT" (Phase 5) feature.
fn format_session_txt(session: &Session) -> String {
    let mut out = format!("Session {}\nRecorded: {}\n", session.id, session.created_at);

    let language = session.language.as_deref().unwrap_or("unknown");
    out.push_str(&format!("Language: {language}\n"));

    out.push_str("\nTranscript:\n");
    out.push_str(&session.transcript);
    out.push('\n');

    if let Some(translation) = &session.translation {
        let target = session.target_lang.as_deref().unwrap_or("unknown");
        out.push_str(&format!("\nTranslation ({target}):\n"));
        out.push_str(translation);
        out.push('\n');
    }

    out
}

/// Opens a native "Save As" dialog and writes the session's transcript (and
/// translation, if any) as plain text. Returns `Ok(None)` if the user
/// cancels the dialog rather than treating cancellation as an error.
#[tauri::command]
pub async fn export_session_txt(
    app: tauri::AppHandle,
    pool: State<'_, SqlitePool>,
    id: String,
) -> Result<Option<String>, String> {
    let repository = SessionRepository::new(pool.inner().clone());
    let session = repository
        .get(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "session not found".to_string())?;

    let content = format_session_txt(&session);
    let default_name = format!("session-{}.txt", session.id);

    let chosen = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_file_name(&default_name)
            .add_filter("Text", &["txt"])
            .blocking_save_file()
    })
    .await
    .map_err(|e| e.to_string())?;

    let Some(file_path) = chosen else {
        return Ok(None);
    };

    let path = file_path.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())?;

    Ok(Some(path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_session() -> Session {
        Session {
            id: "abc-123".to_string(),
            created_at: "2026-08-05T12:00:00Z".to_string(),
            duration_ms: 1_500,
            language: Some("en".to_string()),
            transcript: "hello world".to_string(),
            translation: None,
            target_lang: None,
            synced: 0,
            status: crate::storage::models::SESSION_STATUS_COMPLETE.to_string(),
        }
    }

    #[test]
    fn format_session_txt_without_translation() {
        let session = sample_session();
        let text = format_session_txt(&session);

        assert!(text.contains("Session abc-123"));
        assert!(text.contains("Recorded: 2026-08-05T12:00:00Z"));
        assert!(text.contains("Language: en"));
        assert!(text.contains("Transcript:\nhello world"));
        assert!(!text.contains("Translation"));
    }

    #[test]
    fn format_session_txt_with_translation() {
        let mut session = sample_session();
        session.translation = Some("ola mundo".to_string());
        session.target_lang = Some("pt".to_string());

        let text = format_session_txt(&session);

        assert!(text.contains("Translation (pt):\nola mundo"));
    }

    #[test]
    fn format_session_txt_unknown_language_and_target() {
        let mut session = sample_session();
        session.language = None;
        session.translation = Some("ola mundo".to_string());
        session.target_lang = None;

        let text = format_session_txt(&session);

        assert!(text.contains("Language: unknown"));
        assert!(text.contains("Translation (unknown):\nola mundo"));
    }
}
