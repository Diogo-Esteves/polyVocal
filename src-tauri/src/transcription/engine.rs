use anyhow::Result;
use std::path::PathBuf;

/// Wrapper around a loaded whisper-rs model.
pub struct TranscriptionEngine {
    // TODO: hold whisper_rs::WhisperContext here
    model_path: PathBuf,
}

impl TranscriptionEngine {
    /// Load a Whisper model from disk.
    pub fn load(model_path: PathBuf) -> Result<Self> {
        // TODO: whisper_rs::WhisperContext::new(model_path.to_str().unwrap())
        Ok(Self { model_path })
    }

    /// Transcribe a buffer of 16 kHz mono f32 PCM samples.
    ///
    /// Returns the transcript text and detected language code (e.g. "en", "pt", "es").
    pub fn transcribe(&self, _pcm: &[f32]) -> Result<TranscriptResult> {
        // TODO: set up WhisperParams, run ctx.full(), extract segments
        Ok(TranscriptResult {
            text: String::new(),
            language: "en".into(),
            segments: vec![],
        })
    }
}

#[derive(Debug)]
pub struct TranscriptResult {
    pub text: String,
    pub language: String,
    pub segments: Vec<Segment>,
}

#[derive(Debug)]
pub struct Segment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
}
