use anyhow::Result;
use chrono::Utc;
use uuid::Uuid;

/// A single recording + transcription session.
pub struct TranscriptionSession {
    pub id: Uuid,
    pub started_at: chrono::DateTime<Utc>,
    pub transcript: String,
    pub detected_language: Option<String>,
}

impl TranscriptionSession {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            started_at: Utc::now(),
            transcript: String::new(),
            detected_language: None,
        }
    }

    pub fn append(&mut self, text: &str, language: &str) {
        if !self.transcript.is_empty() {
            self.transcript.push(' ');
        }
        self.transcript.push_str(text.trim());
        self.detected_language = Some(language.to_string());
    }
}

impl Default for TranscriptionSession {
    fn default() -> Self {
        Self::new()
    }
}
