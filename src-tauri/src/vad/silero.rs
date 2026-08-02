#![allow(dead_code)]

use super::segmenter::VoiceActivityScorer;
use anyhow::{anyhow, Result};
use std::path::PathBuf;

/// Silero VAD's recurrent model requires a fixed input width: 512 samples
/// (32 ms) per inference step at 16 kHz.
pub const SILERO_FRAME_SIZE: usize = 512;

/// Wrapper around a loaded Silero VAD ONNX model.
///
/// Actual inference goes through `ort`; wiring the `ort::Session` is deferred
/// until the model file and audio fixtures needed to verify it are in place
/// (see docs/SPEC.md #15 Testing Strategy). Until then, `score` validates its
/// input and returns a safe stub value.
#[derive(Debug)]
pub struct SileroVad {
    model_path: PathBuf,
}

impl SileroVad {
    /// Load a Silero VAD model from disk.
    pub fn load(model_path: PathBuf) -> Result<Self> {
        if !model_path.exists() {
            return Err(anyhow!(
                "Silero VAD model not found: {}",
                model_path.display()
            ));
        }
        // TODO: ort::session::Session::builder()?.commit_from_file(&model_path)
        Ok(Self { model_path })
    }
}

impl VoiceActivityScorer for SileroVad {
    /// Score a single frame for speech probability.
    ///
    /// # Errors
    /// Returns an error if `frame` is not exactly `SILERO_FRAME_SIZE` samples —
    /// Silero's recurrent model requires a fixed input width.
    fn score(&mut self, frame: &[f32]) -> Result<f32> {
        if frame.len() != SILERO_FRAME_SIZE {
            return Err(anyhow!(
                "Silero VAD expects frames of {} samples, got {}",
                SILERO_FRAME_SIZE,
                frame.len()
            ));
        }
        // TODO: run ort session inference, carrying recurrent h/c state across calls
        Ok(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes an empty file at a throwaway path so `load`'s existence check
    /// passes without depending on a real Silero model being present.
    fn dummy_model_path(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, b"").expect("failed to write dummy model file");
        path
    }

    #[test]
    fn test_load_missing_model_fails() {
        let result = SileroVad::load(PathBuf::from("/nonexistent/path/to/silero_vad.onnx"));

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_score_rejects_wrong_frame_size() {
        let path = dummy_model_path("polyvocal_test_silero_wrong_size.onnx");
        let mut vad = SileroVad::load(path).expect("dummy model file should load");

        let wrong_size_frame = vec![0.0f32; SILERO_FRAME_SIZE - 1];
        let result = vad.score(&wrong_size_frame);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("512"));
    }

    #[test]
    fn test_score_accepts_correct_frame_size() {
        let path = dummy_model_path("polyvocal_test_silero_correct_size.onnx");
        let mut vad = SileroVad::load(path).expect("dummy model file should load");

        let frame = vec![0.0f32; SILERO_FRAME_SIZE];
        assert!(vad.score(&frame).is_ok());
    }
}
