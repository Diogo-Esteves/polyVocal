//! Integration test for the audio pipeline, per docs/SPEC.md #15 Testing
//! Strategy / DEC-019: feed a known audio fixture through VAD gating and
//! real whisper-rs inference, and assert the transcript is recognizable.
//!
//! Uses `fixtures/jfk.wav` — whisper.cpp's own well-known public-domain test
//! sample (JFK's "ask not what your country can do for you"), already 16kHz
//! mono PCM. Runs as part of the default `cargo test` (not `--ignored`) per
//! DEC-019's explicit intent ("Integration test suite runs `cargo test` — no
//! special tooling needed"), downloading real models on first run (cached
//! across runs, same infra as the unit-level real-model tests).

use polyvocal_lib::audio::chunker::FrameChunker;
use polyvocal_lib::models::downloader::ReqwestDownloader;
use polyvocal_lib::models::manager::ModelManager;
use polyvocal_lib::models::registry::{ModelSize, VadModel};
use polyvocal_lib::transcription::engine::TranscriptionEngine;
use polyvocal_lib::transcription::pipeline::RecordingPipeline;
use polyvocal_lib::vad::segmenter::SpeechSegmenter;
use polyvocal_lib::vad::silero::{SileroVad, SILERO_FRAME_SIZE};

const VAD_THRESHOLD: f32 = 0.5;
const VAD_MIN_SILENCE_FRAMES: usize = 10;
const VAD_MAX_SEGMENT_FRAMES: usize = 938;

#[tokio::test]
async fn test_jfk_fixture_transcribes_recognizable_speech() {
    let fixture_path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/jfk.wav");
    let mut reader =
        hound::WavReader::open(fixture_path).expect("fixture should be a valid WAV file");
    let spec = reader.spec();
    assert_eq!(spec.sample_rate, 16000, "fixture must already be 16kHz");
    assert_eq!(spec.channels, 1, "fixture must already be mono");

    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.expect("failed to read sample") as f32 / 32768.0)
        .collect();

    let models_dir = std::env::temp_dir().join("polyvocal_integration_test_models");
    let manager = ModelManager::new(models_dir.clone());
    manager
        .download(&ModelSize::Tiny, &ReqwestDownloader)
        .await
        .expect("whisper model should download");
    let vad_model_path = manager
        .ensure_vad_model(&VadModel::Silero, &ReqwestDownloader)
        .await
        .expect("silero model should download");

    let engine = TranscriptionEngine::load(models_dir.join(ModelSize::Tiny.filename()))
        .expect("whisper model should load");
    let scorer = SileroVad::load(vad_model_path).expect("silero model should load");
    let segmenter = SpeechSegmenter::new(
        scorer,
        VAD_THRESHOLD,
        VAD_MIN_SILENCE_FRAMES,
        VAD_MAX_SEGMENT_FRAMES,
    );
    let mut pipeline = RecordingPipeline::new(segmenter);

    // The pipeline no longer transcribes internally (issue #45) — it hands
    // back closed segments and the caller drives the engine. In the app
    // that's `spawn_blocking`; here, on a test thread, calling straight
    // through is fine.
    let mut chunker = FrameChunker::new(SILERO_FRAME_SIZE);
    for chunk in samples.chunks(1600) {
        for frame in chunker.push(chunk) {
            let closed = pipeline
                .push_frame(&frame)
                .expect("pipeline should process real speech audio");
            if let Some(segment_samples) = closed {
                let result = engine
                    .transcribe(&segment_samples)
                    .expect("real speech segment should transcribe");
                pipeline.record_transcript(&result.text, &result.language);
            }
        }
    }

    let session = pipeline.finish();
    let transcript = session.transcript.to_lowercase();

    // Checked as separate distinctive phrases rather than one long exact
    // quote: VAD-gated segments are transcribed independently, so a segment
    // boundary landing mid-word (e.g. splitting "ask" as its own segment)
    // is expected, real behavior — not a bug — and shouldn't fail this test.
    for phrase in [
        "fellow americans",
        "what your country can do for you",
        "what you can do for your country",
    ] {
        assert!(
            transcript.contains(phrase),
            "expected {phrase:?} in transcript, got: {transcript:?}"
        );
    }
}
