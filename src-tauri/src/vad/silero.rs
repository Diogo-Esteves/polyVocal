use super::segmenter::VoiceActivityScorer;
use anyhow::{anyhow, Result};
use ort::session::Session;
use ort::value::TensorRef;
use std::path::PathBuf;

/// Silero VAD's recurrent model requires a fixed input width: 512 samples
/// (32 ms) per inference step at 16 kHz.
pub const SILERO_FRAME_SIZE: usize = 512;
const SILERO_SAMPLE_RATE: i64 = 16000;

/// Length of Silero's combined recurrent state tensor: shape `[2, 1, 128]`
/// (verified against the real model's I/O metadata — v5 combines the old
/// v4 h/c state pair into a single tensor).
const STATE_LEN: usize = 2 * 128;

/// Silero's model expects each 512-sample frame prefixed with the trailing
/// 64 samples of the *previous* frame (a causal convolution look-back
/// window). This isn't visible in the ONNX graph's shape metadata at all —
/// it's a convention only documented in Silero's own Python reference
/// wrapper (`OnnxWrapper.__call__` in the `silero-vad` PyPI package).
/// Omitting it doesn't error; it silently produces near-zero scores for
/// real speech, which is what led to finding this in the first place.
const CONTEXT_SIZE: usize = 64;

/// Wrapper around a loaded Silero VAD ONNX model, scored via `ort`.
#[derive(Debug)]
pub struct SileroVad {
    session: Session,
    /// Recurrent state carried between calls, shape `[2, 1, 128]` flattened.
    state: Vec<f32>,
    /// Trailing `CONTEXT_SIZE` samples of the previously scored frame,
    /// prepended to the next input (see `CONTEXT_SIZE` doc comment).
    context: Vec<f32>,
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

        let session = Session::builder()
            .map_err(|e| anyhow!("failed to create ONNX Runtime session builder: {e}"))?
            .commit_from_file(&model_path)
            .map_err(|e| anyhow!("failed to load Silero VAD model: {e}"))?;

        Ok(Self {
            session,
            state: vec![0.0; STATE_LEN],
            context: vec![0.0; CONTEXT_SIZE],
        })
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

        let mut windowed = Vec::with_capacity(CONTEXT_SIZE + SILERO_FRAME_SIZE);
        windowed.extend_from_slice(&self.context);
        windowed.extend_from_slice(frame);

        let input = TensorRef::from_array_view((
            [1_i64, (CONTEXT_SIZE + SILERO_FRAME_SIZE) as i64],
            windowed.as_slice(),
        ))
        .map_err(|e| anyhow!("failed to build Silero input tensor: {e}"))?;
        let state_tensor = TensorRef::from_array_view(([2_i64, 1, 128], self.state.as_slice()))
            .map_err(|e| anyhow!("failed to build Silero state tensor: {e}"))?;
        let sr = TensorRef::from_array_view((Vec::<i64>::new(), &[SILERO_SAMPLE_RATE][..]))
            .map_err(|e| anyhow!("failed to build Silero sample-rate tensor: {e}"))?;

        let outputs = self
            .session
            .run(ort::inputs!["input" => input, "state" => state_tensor, "sr" => sr])
            .map_err(|e| anyhow!("Silero VAD inference failed: {e}"))?;

        let (_, probability) = outputs["output"]
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow!("failed to read Silero output tensor: {e}"))?;
        let probability = *probability
            .first()
            .ok_or_else(|| anyhow!("Silero VAD returned an empty output tensor"))?;

        let (_, new_state) = outputs["stateN"]
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow!("failed to read Silero state tensor: {e}"))?;
        self.state.copy_from_slice(new_state);

        self.context
            .copy_from_slice(&frame[frame.len() - CONTEXT_SIZE..]);

        Ok(probability)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_missing_model_fails() {
        let result = SileroVad::load(PathBuf::from("/nonexistent/path/to/silero_vad.onnx"));

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_load_rejects_invalid_model_file() {
        // A real ort Session can't be built from arbitrary bytes — ONNX
        // Runtime validates the file format itself. Unlike the missing-model
        // case above, this exercises the "exists but isn't a real model" path.
        let path = std::env::temp_dir().join("polyvocal_test_silero_invalid_model.onnx");
        std::fs::write(&path, b"not a real onnx model").expect("failed to write test file");

        let result = SileroVad::load(path);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("failed to load Silero VAD model"));
    }

    /// Downloads the real Silero model (cached across runs, reusing the
    /// Phase 5 download infra) and runs real inference against it — not
    /// part of the default suite (network + download on first run), run
    /// manually with `--ignored` when touching this file. Verifies the
    /// actual ONNX I/O contract (input/state/sr in, output/stateN out),
    /// that recurrent state carries correctly across calls, AND that real
    /// speech (not just silence) scores as speech — silence-only scoring
    /// low is a necessary but not sufficient check, since a broken context
    /// window (see CONTEXT_SIZE) previously passed a silence-only version
    /// of this test while scoring real speech as near-zero too.
    #[tokio::test]
    #[ignore]
    async fn test_real_silero_inference_end_to_end() {
        use crate::models::downloader::ReqwestDownloader;
        use crate::models::manager::ModelManager;
        use crate::models::registry::VadModel;

        let models_dir = std::env::temp_dir().join("polyvocal_test_real_silero_models");
        let manager = ModelManager::new(models_dir);
        let model_path = manager
            .ensure_vad_model(&VadModel::Silero, &ReqwestDownloader)
            .await
            .expect("real Silero model should download");

        let mut vad = SileroVad::load(model_path).expect("real model should load");

        // Wrong frame size is rejected before touching the model at all.
        assert!(vad.score(&[0.0; SILERO_FRAME_SIZE - 1]).is_err());

        // Silence should score as (very likely) not speech, and scoring
        // multiple frames in a row must not fail — proves the recurrent
        // state tensor round-trips correctly between calls.
        let silence = vec![0.0f32; SILERO_FRAME_SIZE];
        let mut last_probability = 1.0;
        for _ in 0..5 {
            last_probability = vad
                .score(&silence)
                .expect("scoring silence should not fail");
            assert!((0.0..=1.0).contains(&last_probability));
        }
        assert!(
            last_probability < 0.5,
            "expected silence to score as not-speech, got {last_probability}"
        );

        // A loud synthetic tone should score as speech-like once the
        // context window has real signal in it — proves non-silent input
        // actually reaches the model and produces a high score — a pure
        // synthetic tone was tried here first and rejected: Silero is
        // trained on real speech spectra, and a tone maxes out around 0.3
        // regardless of amplitude, which isn't a reliable "is this working"
        // signal. Real speech is the only trustworthy positive case.
        let fixture_path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/jfk.wav");
        let mut reader =
            hound::WavReader::open(fixture_path).expect("fixture should be a valid WAV file");
        let samples: Vec<f32> = reader
            .samples::<i16>()
            .map(|s| s.expect("failed to read sample") as f32 / 32768.0)
            .collect();

        let mut max_speech_score: f32 = 0.0;
        for frame in samples.as_chunks::<SILERO_FRAME_SIZE>().0 {
            let score = vad
                .score(frame)
                .expect("scoring real speech should not fail");
            max_speech_score = max_speech_score.max(score);
        }
        assert!(
            max_speech_score > 0.9,
            "expected real speech to score confidently as speech, got max {max_speech_score}"
        );
    }
}
