use anyhow::{anyhow, Result};
use std::path::PathBuf;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Wrapper around a loaded whisper-rs model.
#[derive(Debug)]
pub struct TranscriptionEngine {
    context: WhisperContext,
}

impl TranscriptionEngine {
    /// Load a Whisper model from disk.
    pub fn load(model_path: PathBuf) -> Result<Self> {
        if !model_path.exists() {
            return Err(anyhow!("Whisper model not found: {}", model_path.display()));
        }

        let context =
            WhisperContext::new_with_params(&model_path, WhisperContextParameters::default())
                .map_err(|e| anyhow!("failed to load Whisper model: {e}"))?;

        Ok(Self { context })
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

        let mut state = self
            .context
            .create_state()
            .map_err(|e| anyhow!("failed to create Whisper state: {e}"))?;

        // Greedy, not beam search: beam_size 5 is ~5x the compute per segment,
        // which pushed live transcription below real-time on typical
        // hardware and starved the capture channel (see #144). Beam search
        // stays a candidate for a future non-live "re-transcribe for
        // accuracy" path, not the live-capture default.
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 5 });
        params.set_language(None); // auto-detect, per DEC-003
                                   // whisper-rs defaults to 4 threads regardless of what's available:
                                   // that's what a "25% CPU, still can't keep up live" reading on a
                                   // 16-core machine actually was (see #144) — under-using the
                                   // hardware while still falling behind it. Capped at 8 rather than
                                   // using every core: whisper.cpp's own thread scaling flattens out
                                   // well before that on typical consumer core counts, and this task
                                   // shares the machine with audio capture and VAD.
        let n_threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(8) as std::os::raw::c_int;
        params.set_n_threads(n_threads);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        state
            .full(params, pcm)
            .map_err(|e| anyhow!("whisper inference failed: {e}"))?;

        let language = whisper_rs::get_lang_str(state.full_lang_id_from_state())
            .unwrap_or("en")
            .to_string();

        let mut text = String::new();
        let mut segments = Vec::new();
        for segment in state.as_iter() {
            let segment_text = segment.to_str_lossy().unwrap_or_default().into_owned();
            if !text.is_empty() && !segment_text.is_empty() {
                text.push(' ');
            }
            text.push_str(segment_text.trim());
            segments.push(Segment {
                // whisper.cpp timestamps are in centiseconds (10s of ms).
                start_ms: segment.start_timestamp() * 10,
                end_ms: segment.end_timestamp() * 10,
                text: segment_text,
            });
        }

        Ok(TranscriptResult {
            text,
            language,
            segments,
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

    #[test]
    fn test_load_missing_model_fails() {
        let result = TranscriptionEngine::load(PathBuf::from("/nonexistent/path/to/ggml-tiny.bin"));

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_load_rejects_invalid_model_file() {
        // A real WhisperContext can't be loaded from arbitrary bytes — whisper.cpp
        // validates the file format itself. Unlike the missing-model case above,
        // this exercises the "exists but isn't a real model" error path.
        let path = std::env::temp_dir().join("polyvocal_test_whisper_invalid_model.bin");
        std::fs::write(&path, b"not a real ggml model").expect("failed to write test file");

        let result = TranscriptionEngine::load(path);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("failed to load Whisper model"));
    }

    /// Downloads the real tiny Whisper model (cached across runs, reusing
    /// the Phase 5 download infra) and runs real inference against it — not
    /// part of the default suite (network + a ~75MB download on first run),
    /// run manually with `--ignored` when touching whisper-rs wiring. Proves
    /// the FFI pipeline works end-to-end; not a transcription-accuracy test,
    /// since we don't have real speech audio fixtures (silence in, here).
    #[tokio::test]
    #[ignore]
    async fn test_real_whisper_inference_end_to_end() {
        use crate::models::downloader::ReqwestDownloader;
        use crate::models::manager::ModelManager;
        use crate::models::registry::ModelSize;

        let models_dir = std::env::temp_dir().join("polyvocal_test_real_whisper_models");
        let manager = ModelManager::new(models_dir.clone());
        manager
            .download(&ModelSize::Tiny, &ReqwestDownloader)
            .await
            .expect("real tiny model should download");

        let model_path = models_dir.join(ModelSize::Tiny.filename());
        let engine = TranscriptionEngine::load(model_path).expect("real model should load");

        assert!(engine.transcribe(&[]).is_err());

        let pcm = vec![0.0f32; 16000]; // 1 second of silence at 16kHz
        let result = engine
            .transcribe(&pcm)
            .expect("real inference should not fail on silence");

        // Silence isn't real speech, so we can't assert on transcript
        // accuracy — only that the FFI round-trip produced a well-formed
        // result without crashing.
        assert!(!result.language.is_empty());
    }
}
