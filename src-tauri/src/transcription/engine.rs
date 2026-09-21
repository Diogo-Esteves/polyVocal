use anyhow::{anyhow, Result};
use std::path::PathBuf;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Decoding knobs for a single `transcribe` call — kept out of
/// `TranscriptionEngine` itself so callers (e.g. the live-transcription
/// worker in `commands::audio`) can vary strategy per segment without
/// touching the engine, and so a future adaptive-tier selector (#144 Phase 2)
/// or forced-language option (#22) can be threaded through without an engine
/// API change.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodeOptions {
    pub strategy: DecodeStrategy,
    pub n_threads: std::os::raw::c_int,
    /// `None` = auto-detect (whisper.cpp default, DEC-003).
    pub language: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeStrategy {
    /// ~5x cheaper than beam search; the live-capture default (see #144).
    Greedy,
    BeamSearch {
        beam_size: std::os::raw::c_int,
    },
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            strategy: DecodeStrategy::Greedy,
            n_threads: default_n_threads(),
            language: None,
        }
    }
}

/// whisper-rs defaults to 4 threads regardless of what's available: that's
/// what a "25% CPU, still can't keep up live" reading on a 16-core machine
/// actually was (see #144) — under-using the hardware while still falling
/// behind it. Capped at 8 rather than using every core: whisper.cpp's own
/// thread scaling flattens out well before that on typical consumer core
/// counts, and this task shares the machine with audio capture and VAD.
pub fn default_n_threads() -> std::os::raw::c_int {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8) as std::os::raw::c_int
}

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
    pub fn transcribe(&self, pcm: &[f32], options: &DecodeOptions) -> Result<TranscriptResult> {
        if pcm.is_empty() {
            return Err(anyhow!("Cannot transcribe empty audio buffer"));
        }

        let mut state = self
            .context
            .create_state()
            .map_err(|e| anyhow!("failed to create Whisper state: {e}"))?;

        let mut params = match options.strategy {
            DecodeStrategy::Greedy => FullParams::new(SamplingStrategy::Greedy { best_of: 5 }),
            DecodeStrategy::BeamSearch { beam_size } => {
                FullParams::new(SamplingStrategy::BeamSearch {
                    beam_size,
                    patience: -1.0,
                })
            }
        };
        params.set_language(options.language.as_deref());
        params.set_n_threads(options.n_threads);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_token_timestamps(true);

        state
            .full(params, pcm)
            .map_err(|e| anyhow!("whisper inference failed: {e}"))?;

        let language = whisper_rs::get_lang_str(state.full_lang_id_from_state())
            .unwrap_or("en")
            .to_string();

        let mut text = String::new();
        let mut segments = Vec::new();
        let token_eot = self.context.token_eot();
        for segment in state.as_iter() {
            let segment_text = segment.to_str_lossy().unwrap_or_default().into_owned();
            if !text.is_empty() && !segment_text.is_empty() {
                text.push(' ');
            }
            text.push_str(segment_text.trim());

            // Extract word-level tokens from this segment.
            let mut words = Vec::new();
            let n_tokens = segment.n_tokens();
            for i in 0..n_tokens {
                if let Some(token) = segment.get_token(i) {
                    // Skip special/control tokens (EOT, language, timestamp markers, etc.)
                    if token.token_id() >= token_eot {
                        continue;
                    }
                    // Extract token text — skip on error rather than failing.
                    let token_text = match token.to_str_lossy() {
                        Ok(text) => text,
                        Err(_) => continue,
                    };
                    let trimmed = token_text.trim().to_string();
                    // Skip empty tokens (whitespace-only).
                    if trimmed.is_empty() {
                        continue;
                    }
                    // Extract timing via token_data (centiseconds, same as segment timestamps).
                    let token_data = token.token_data();
                    words.push(Word {
                        text: trimmed,
                        start_ms: token_data.t0 * 10,
                        end_ms: token_data.t1 * 10,
                    });
                }
            }

            segments.push(Segment {
                // whisper.cpp timestamps are in centiseconds (10s of ms).
                start_ms: segment.start_timestamp() * 10,
                end_ms: segment.end_timestamp() * 10,
                text: segment_text,
                words,
            });
        }

        Ok(TranscriptResult {
            text,
            language,
            segments,
        })
    }
}

#[derive(Debug, Clone)]
pub struct TranscriptResult {
    pub text: String,
    pub language: String,
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone)]
pub struct Word {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug, Clone)]
pub struct Segment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub words: Vec<Word>,
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

        assert!(engine.transcribe(&[], &DecodeOptions::default()).is_err());

        let pcm = vec![0.0f32; 16000]; // 1 second of silence at 16kHz
        let result = engine
            .transcribe(&pcm, &DecodeOptions::default())
            .expect("real inference should not fail on silence");

        // Silence isn't real speech, so we can't assert on transcript
        // accuracy — only that the FFI round-trip produced a well-formed
        // result without crashing.
        assert!(!result.language.is_empty());
    }

    /// Tests word-level timestamp extraction from real Whisper inference.
    /// Verifies that tokens are properly filtered, timing is correct, and
    /// word timestamps are scoped to their parent segment.
    #[tokio::test]
    #[ignore]
    async fn test_real_whisper_word_level_timestamps() {
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

        let pcm = vec![0.0f32; 16000]; // 1 second of silence at 16kHz
        let result = engine
            .transcribe(&pcm, &DecodeOptions::default())
            .expect("real inference should not fail");

        // Verify segments exist and at least one has words.
        assert!(
            !result.segments.is_empty(),
            "result should have at least one segment"
        );
        let has_words = result.segments.iter().any(|seg| !seg.words.is_empty());
        assert!(has_words, "at least one segment should have words");

        // Verify word timestamps are valid and scoped to their segment.
        for segment in &result.segments {
            for word in &segment.words {
                assert!(
                    word.start_ms <= word.end_ms,
                    "word start_ms must be <= end_ms: {word:?}"
                );
                assert!(
                    word.start_ms >= segment.start_ms,
                    "word start_ms must be >= segment.start_ms: word={word:?}, segment={segment:?}"
                );
                assert!(
                    word.end_ms <= segment.end_ms,
                    "word end_ms must be <= segment.end_ms: word={word:?}, segment={segment:?}"
                );
            }
        }
    }
}
