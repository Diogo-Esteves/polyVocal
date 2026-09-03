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
use polyvocal_lib::transcription::engine::{DecodeOptions, TranscriptionEngine};
use polyvocal_lib::transcription::pipeline::RecordingPipeline;
use polyvocal_lib::transcription::session::TranscriptionSession;
use polyvocal_lib::vad::segmenter::SpeechSegmenter;
use polyvocal_lib::vad::silero::{SileroVad, SILERO_FRAME_SIZE};
// The production defaults, imported rather than redeclared, so this test
// can't silently drift from what `start_recording` actually uses.
use polyvocal_lib::vad::{VAD_MAX_SEGMENT_FRAMES, VAD_MIN_SILENCE_FRAMES, VAD_THRESHOLD};

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
    let mut session = TranscriptionSession::new();

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
            if let Some(segment) = closed {
                let result = engine
                    .transcribe(&segment.samples, &DecodeOptions::default())
                    .expect("real speech segment should transcribe");
                session.append(&result.text, &result.language);
            }
        }
    }
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

/// Integration test for Portuguese language detection and transcription.
///
/// Uses `fixtures/pt_conto.wav` — a ~12s clip from a LibriVox Portuguese
/// recording of "Contos Infantis" (Grimm fairy tales translated to Portuguese),
/// track `contosinfantis_06_grimm_64kb.mp3`, from archive.org item
/// `perolas_e_diamantes_2007_librivox`
/// (https://archive.org/details/perolas_e_diamantes_2007_librivox).
/// LibriVox recordings are dedicated to the public domain by both the reader
/// and LibriVox itself — same public-domain basis as `jfk.wav`, just a different
/// source. The clip is LibriVox's own standard recording announcement, not the
/// story itself. 16kHz mono PCM, runs as part of the default `cargo test`.
#[tokio::test]
async fn test_pt_fixture_detects_portuguese_and_transcribes_recognizable_speech() {
    let fixture_path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/pt_conto.wav");
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
    let mut session = TranscriptionSession::new();

    let mut chunker = FrameChunker::new(SILERO_FRAME_SIZE);
    let mut detected_language: Option<String> = None;
    for chunk in samples.chunks(1600) {
        for frame in chunker.push(chunk) {
            let closed = pipeline
                .push_frame(&frame)
                .expect("pipeline should process real speech audio");
            if let Some(segment) = closed {
                let result = engine
                    .transcribe(&segment.samples, &DecodeOptions::default())
                    .expect("real speech segment should transcribe");
                if !result.language.is_empty() {
                    detected_language = Some(result.language.clone());
                }
                session.append(&result.text, &result.language);
            }
        }
    }
    let transcript = session.transcript.to_lowercase();

    // Checked as separate distinctive phrases rather than one long exact
    // quote: VAD-gated segments are transcribed independently, so a segment
    // boundary landing mid-word is expected, real behavior — not a bug —
    // and shouldn't fail this test.
    for phrase in ["gravação", "domínio público"] {
        assert!(
            transcript.contains(phrase),
            "expected {phrase:?} in transcript, got: {transcript:?}"
        );
    }

    assert_eq!(
        detected_language,
        Some("pt".to_string()),
        "expected Portuguese language detection"
    );
}

/// Integration test for Spanish language detection and transcription.
///
/// Uses `fixtures/es_fabula.wav` — a ~12s clip from a LibriVox Spanish
/// recording of Aesop's Fables ("Las Fábulas de Esopo"), specifically fable 61
/// "El lobo y el cordero" (The Wolf and the Lamb), track
/// `fabula_03_061_esopo_64kb.mp3`, from archive.org item `fabulas_esopo_03_librivox`
/// (https://archive.org/details/fabulas_esopo_03_librivox). LibriVox recordings are
/// dedicated to the public domain by both the reader and LibriVox itself — same
/// public-domain basis as `jfk.wav`, just a different source. 16kHz mono PCM,
/// runs as part of the default `cargo test`.
#[tokio::test]
async fn test_es_fixture_detects_spanish_and_transcribes_recognizable_speech() {
    let fixture_path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/es_fabula.wav");
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
    let mut session = TranscriptionSession::new();

    let mut chunker = FrameChunker::new(SILERO_FRAME_SIZE);
    let mut detected_language: Option<String> = None;
    for chunk in samples.chunks(1600) {
        for frame in chunker.push(chunk) {
            let closed = pipeline
                .push_frame(&frame)
                .expect("pipeline should process real speech audio");
            if let Some(segment) = closed {
                let result = engine
                    .transcribe(&segment.samples, &DecodeOptions::default())
                    .expect("real speech segment should transcribe");
                if !result.language.is_empty() {
                    detected_language = Some(result.language.clone());
                }
                session.append(&result.text, &result.language);
            }
        }
    }
    let transcript = session.transcript.to_lowercase();

    // Single distinctive phrase check: VAD-gated segments have different
    // acoustic context, so marginal words on the low-accuracy tiny model
    // can flip (e.g. templo/temple) between runs depending on segment
    // boundaries. One correctly-transcribed, distinctive word plus correct
    // language ID is sufficient signal that real Spanish speech was recognized.
    assert!(
        transcript.contains("cordero"),
        "expected 'cordero' in transcript, got: {transcript:?}"
    );

    assert_eq!(
        detected_language,
        Some("es".to_string()),
        "expected Spanish language detection"
    );
}
