#![allow(dead_code)]

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A session that is still being recorded — segments are being appended to
/// it, but it hasn't been finalised by `stop_recording` yet. A session still
/// in this state at launch was interrupted (crash, OOM, force-quit).
pub const SESSION_STATUS_IN_PROGRESS: &str = "in_progress";
/// A session finalised cleanly on stop, with its total duration recorded.
pub const SESSION_STATUS_COMPLETE: &str = "complete";

/// A transcription session stored in the database.
///
/// `transcript` is a denormalised cache of this session's `segments`, kept
/// up to date as each segment is written (see
/// `SessionRepository::append_segment`) — `segments` remains the
/// authoritative, timestamped record.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Session {
    pub id: String,
    pub created_at: String,
    pub duration_ms: i64,
    pub language: Option<String>,
    pub transcript: String,
    pub translation: Option<String>,
    pub target_lang: Option<String>,
    /// Whether this session has been synced to another device (0 = no, 1 = yes).
    pub synced: i64,
    /// `in_progress` or `complete` — see the `SESSION_STATUS_*` constants.
    pub status: String,
}

impl Session {
    pub fn new(transcript: String, language: Option<String>, duration_ms: i64) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            created_at: Utc::now().to_rfc3339(),
            duration_ms,
            language,
            transcript,
            translation: None,
            target_lang: None,
            synced: 0,
            status: SESSION_STATUS_COMPLETE.to_string(),
        }
    }
}

/// One VAD-closed speech segment of a session, persisted as soon as it has
/// been transcribed (DEC-009).
///
/// `start_ms`/`end_ms` are offsets from the start of the recording, so
/// segments of one session are directly comparable (and usable for SRT
/// export) — unlike whisper's own timestamps, which restart at zero for
/// every buffer handed to it.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TranscriptSegment {
    pub id: String,
    pub session_id: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub language: Option<String>,
}

impl TranscriptSegment {
    pub fn new(
        session_id: &str,
        text: &str,
        language: Option<&str>,
        start_ms: i64,
        end_ms: i64,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            start_ms,
            end_ms,
            text: text.trim().to_string(),
            language: language.map(str::to_string),
        }
    }
}
