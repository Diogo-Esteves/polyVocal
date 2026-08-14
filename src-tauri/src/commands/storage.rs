use crate::storage::models::{Session, TranscriptSegment};
use crate::storage::repository::SessionRepository;
use sqlx::SqlitePool;
use tauri::State;
use tauri_plugin_dialog::DialogExt;
use tracing::warn;

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

/// Formats a session-relative offset in milliseconds as an SRT timestamp,
/// `HH:MM:SS,mmm` — comma before the milliseconds, per the SRT spec (WebVTT
/// is the one that uses a period). Hours are not clamped to two digits for
/// recordings longer than 99 hours; nothing shorter overflows.
fn format_srt_timestamp(ms: i64) -> String {
    let ms = ms.max(0);
    let hours = ms / 3_600_000;
    let minutes = (ms % 3_600_000) / 60_000;
    let seconds = (ms % 60_000) / 1_000;
    let millis = ms % 1_000;

    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

/// Renders a session's timestamped segments as an SRT subtitle file.
///
/// Segments whose text is empty are skipped so no malformed (text-less) cue
/// is emitted; the cue index stays sequential over the cues actually written.
/// A session with no usable segments yields an empty string — an empty `.srt`
/// is a valid, if pointless, subtitle file, and failing the export of a
/// crashed/empty session would be less useful than writing nothing.
fn format_session_srt(segments: &[TranscriptSegment]) -> String {
    let mut out = String::new();

    for (index, segment) in segments
        .iter()
        .filter(|segment| !segment.text.trim().is_empty())
        .enumerate()
    {
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            index + 1,
            format_srt_timestamp(segment.start_ms),
            format_srt_timestamp(segment.end_ms),
            segment.text.trim(),
        ));
    }

    out
}

/// Opens a native "Save As" dialog and writes the session's timestamped
/// segments as an SRT subtitle file. Returns `Ok(None)` if the user cancels
/// the dialog rather than treating cancellation as an error.
#[tauri::command]
pub async fn export_session_srt(
    app: tauri::AppHandle,
    pool: State<'_, SqlitePool>,
    id: String,
) -> Result<Option<String>, String> {
    let repository = SessionRepository::new(pool.inner().clone());
    // `segments` can't distinguish a bogus id from a session with nothing
    // transcribed yet, so the existence check still goes through `get`.
    let session = repository
        .get(&id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "session not found".to_string())?;

    let segments = repository
        .segments(&session.id)
        .await
        .map_err(|e| e.to_string())?;

    if segments.is_empty() {
        warn!(session_id = %session.id, "exporting SRT for a session with no segments");
    }

    let content = format_session_srt(&segments);
    let default_name = format!("session-{}.srt", session.id);

    let chosen = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_file_name(&default_name)
            .add_filter("SubRip subtitle", &["srt"])
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

    fn segment(text: &str, start_ms: i64, end_ms: i64) -> TranscriptSegment {
        TranscriptSegment::new("abc-123", text, Some("en"), start_ms, end_ms)
    }

    #[test]
    fn format_srt_timestamp_formats_zero() {
        assert_eq!(format_srt_timestamp(0), "00:00:00,000");
    }

    #[test]
    fn format_srt_timestamp_formats_minutes_seconds_and_millis() {
        assert_eq!(format_srt_timestamp(61_234), "00:01:01,234");
        assert_eq!(format_srt_timestamp(999), "00:00:00,999");
        assert_eq!(format_srt_timestamp(1_000), "00:00:01,000");
    }

    #[test]
    fn format_srt_timestamp_formats_past_an_hour() {
        // 2h 3m 4.005s
        assert_eq!(format_srt_timestamp(7_384_005), "02:03:04,005");
    }

    #[test]
    fn format_srt_timestamp_clamps_negative_offsets() {
        assert_eq!(format_srt_timestamp(-1), "00:00:00,000");
    }

    #[test]
    fn format_session_srt_numbers_cues_sequentially() {
        let segments = vec![
            segment("hello world", 0, 900),
            segment("second cue", 1_200, 61_234),
        ];

        let srt = format_session_srt(&segments);

        assert_eq!(
            srt,
            "1\n00:00:00,000 --> 00:00:00,900\nhello world\n\n\
             2\n00:00:01,200 --> 00:01:01,234\nsecond cue\n\n"
        );
    }

    #[test]
    fn format_session_srt_skips_empty_segments_without_gapping_indices() {
        let segments = vec![
            segment("first", 0, 500),
            segment("   ", 500, 1_000),
            segment("second", 1_000, 1_500),
        ];

        let srt = format_session_srt(&segments);

        assert_eq!(
            srt,
            "1\n00:00:00,000 --> 00:00:00,500\nfirst\n\n\
             2\n00:00:01,000 --> 00:00:01,500\nsecond\n\n"
        );
    }

    #[test]
    fn format_session_srt_with_no_segments_is_empty() {
        assert_eq!(format_session_srt(&[]), "");
    }
}
