use serde::{Deserialize, Serialize};

/// Mirrors the backend `storage::models::Session` struct — only the fields
/// this screen renders are declared; serde ignores the rest (same approach
/// as `TranscriptSegment` above). No `#[serde(rename_all)]` here — unlike
/// the `*Args` structs above (which are command *arguments*, auto-camelCased
/// by Tauri), this is a command *return value*, serialized as-is by the
/// backend's own (unrenamed, snake_case) `Serialize` impl.
#[derive(Deserialize, Clone)]
pub struct Session {
    pub id: String,
    pub created_at: String,
    pub duration_ms: i64,
    pub language: Option<String>,
    pub transcript: String,
    pub translation: Option<String>,
    pub target_lang: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListSessionsArgs {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GetSessionArgs<'a> {
    id: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteSessionArgs<'a> {
    id: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportSessionTxtArgs<'a> {
    id: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportSessionSrtArgs<'a> {
    id: &'a str,
}

/// Lists sessions with a default limit of 20 and offset of 0.
pub async fn list_sessions() -> Result<Vec<Session>, String> {
    let args = ListSessionsArgs {
        limit: Some(20),
        offset: Some(0),
    };
    tauri_sys::core::invoke_result::<Vec<Session>, String>("list_sessions", args).await
}

/// Fetches a specific session by ID.
pub async fn get_session(id: &str) -> Result<Option<Session>, String> {
    let args = GetSessionArgs { id };
    tauri_sys::core::invoke_result::<Option<Session>, String>("get_session", args).await
}

/// Deletes a session by ID.
pub async fn delete_session(id: &str) -> Result<(), String> {
    let args = DeleteSessionArgs { id };
    tauri_sys::core::invoke_result::<(), String>("delete_session", args).await
}

/// Exports a session as plain text.
pub async fn export_session_txt(id: &str) -> Result<Option<String>, String> {
    let args = ExportSessionTxtArgs { id };
    tauri_sys::core::invoke_result::<Option<String>, String>("export_session_txt", args).await
}

/// Exports a session as SRT (SubRip) format.
pub async fn export_session_srt(id: &str) -> Result<Option<String>, String> {
    let args = ExportSessionSrtArgs { id };
    tauri_sys::core::invoke_result::<Option<String>, String>("export_session_srt", args).await
}
