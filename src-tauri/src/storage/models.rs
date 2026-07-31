#![allow(dead_code)]

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A completed transcription session stored in the database.
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
        }
    }
}
