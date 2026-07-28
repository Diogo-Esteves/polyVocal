use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct TranscriptStatus {
    pub is_recording: bool,
    pub partial_text: String,
    pub detected_language: Option<String>,
}

#[tauri::command]
pub async fn get_transcript_status() -> Result<TranscriptStatus, String> {
    // TODO: read from shared app state
    Ok(TranscriptStatus {
        is_recording: false,
        partial_text: String::new(),
        detected_language: None,
    })
}
