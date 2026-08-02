#![allow(dead_code)]

use anyhow::{anyhow, Result};
use std::path::PathBuf;

/// Wrapper around a loaded whisper-rs model.
#[derive(Debug)]
pub struct TranscriptionEngine {
    // TODO: hold whisper_rs::WhisperContext here
    model_path: PathBuf,
}

impl TranscriptionEngine {
    /// Load a Whisper model from disk.
    pub fn load(model_path: PathBuf) -> Result<Self> {
        if !model_path.exists() {
            return Err(anyhow!("Whisper model not found: {}", model_path.display()));
        }
        // TODO: whisper_rs::WhisperContext::new(model_path.to_str().unwrap())
        Ok(Self { model_path })
    }

    /// Transcribe a buffer of 16 kHz mono f32 PCM samples.
    ///
    /// Returns the transcript text and detected language code (e.g. "en", "pt", "es").
    ///
    /// # Errors
    /// Returns an error if `pcm` is empty — there's nothing to transcribe.
    pub fn transcribe(&self, pcm: &[f32]) -> Result<TranscriptResult> {
        if pcm.is_empty() {
            return Err(anyhow!("Cannot transcribe empty audio buffer"));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes an empty file at a throwaway path so `load`'s existence check
    /// passes without depending on a real Whisper model being present.
    fn dummy_model_path(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, b"").expect("failed to write dummy model file");
        path
    }

    #[test]
    fn test_load_missing_model_fails() {
        let result = TranscriptionEngine::load(PathBuf::from("/nonexistent/path/to/ggml-tiny.bin"));

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_transcribe_rejects_empty_audio() {
        let path = dummy_model_path("polyvocal_test_whisper_empty_audio.bin");
        let engine = TranscriptionEngine::load(path).expect("dummy model file should load");

        let result = engine.transcribe(&[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    #[test]
    fn test_transcribe_accepts_nonempty_audio() {
        let path = dummy_model_path("polyvocal_test_whisper_nonempty_audio.bin");
        let engine = TranscriptionEngine::load(path).expect("dummy model file should load");

        let pcm = vec![0.0f32; 1600]; // 100ms of silence at 16kHz
        assert!(engine.transcribe(&pcm).is_ok());
    }
}
