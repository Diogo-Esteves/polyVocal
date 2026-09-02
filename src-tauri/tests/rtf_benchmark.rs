//! Reference RTF (real-time factor) benchmark for #144 Phase 2 —
//! validation plan item 1: measure wall-clock transcribe time vs. audio
//! duration per `(ModelSize, DecodeStrategy)` combination, using audio at
//! the app's actual ~30s VAD segment cap rather than just the short
//! `jfk.wav` fixture alone (whisper pads every buffer to a fixed 30s
//! encoder window regardless of content length, so a benchmark using only
//! the fixture's ~11s clip would hand `transcription::calibration` the
//! wrong numbers).
//!
//! Downloads all four Whisper tiers (~2.4GB combined) and runs real
//! inference several times per tier — not part of the default `cargo test
//! --all`, run manually with:
//!
//!   cargo test --test rtf_benchmark -- --ignored --nocapture
//!
//! following the same convention as the `test_real_candle_translation_*`
//! and `test_real_whisper_inference_end_to_end` real-weights tests (see
//! `.claude/rules/commands.md`).

use polyvocal_lib::models::downloader::ReqwestDownloader;
use polyvocal_lib::models::manager::ModelManager;
use polyvocal_lib::models::registry::ModelSize;
use polyvocal_lib::transcription::calibration::{calibration_pcm, CALIBRATION_AUDIO_SECS};
use polyvocal_lib::transcription::engine::{DecodeOptions, DecodeStrategy, TranscriptionEngine};

#[tokio::test]
#[ignore]
async fn benchmark_rtf_across_model_tiers_and_decode_strategies() {
    tracing_subscriber::fmt::try_init().ok();

    let models_dir = std::env::temp_dir().join("polyvocal_rtf_benchmark_models");
    let manager = ModelManager::new(models_dir.clone());

    let pcm = calibration_pcm().expect("embedded calibration fixture should decode");

    let tiers = [
        ModelSize::Tiny,
        ModelSize::Base,
        ModelSize::Small,
        ModelSize::Medium,
    ];
    let strategies = [
        DecodeStrategy::Greedy,
        DecodeStrategy::BeamSearch { beam_size: 5 },
    ];

    let mut results: Vec<(ModelSize, DecodeStrategy, f64)> = Vec::new();

    for tier in tiers {
        manager
            .download(&tier, &ReqwestDownloader)
            .await
            .unwrap_or_else(|e| panic!("{tier:?} model should download: {e}"));
        let engine = TranscriptionEngine::load(models_dir.join(tier.filename()))
            .unwrap_or_else(|e| panic!("{tier:?} model should load: {e}"));

        for strategy in strategies {
            let options = DecodeOptions {
                strategy,
                ..DecodeOptions::default()
            };
            let start = std::time::Instant::now();
            engine
                .transcribe(&pcm, &options)
                .unwrap_or_else(|e| panic!("{tier:?}/{strategy:?} should transcribe: {e}"));
            let rtf = start.elapsed().as_secs_f64() / CALIBRATION_AUDIO_SECS;

            tracing::info!(?tier, ?strategy, rtf, "RTF benchmark sample");
            results.push((tier, strategy, rtf));
        }
    }

    tracing::info!(
        "=== RTF benchmark summary (lower is faster; <1.0 keeps up with live speech) ==="
    );
    for (tier, strategy, rtf) in &results {
        tracing::info!("{tier:?} / {strategy:?}: RTF = {rtf:.3}");
    }

    // Sanity, not a strict perf gate (machine-dependent): every measured RTF
    // must be a finite, non-negative number, and a smaller tier must not be
    // slower than a larger tier at the same strategy — proves the benchmark
    // loop itself is wired correctly, without asserting fixed thresholds
    // that would make this test flaky across CI/dev hardware.
    for (_, _, rtf) in &results {
        assert!(rtf.is_finite() && *rtf >= 0.0);
    }
    for strategy in strategies {
        let tiny_rtf = results
            .iter()
            .find(|(t, s, _)| *t == ModelSize::Tiny && *s == strategy)
            .map(|(_, _, rtf)| *rtf)
            .unwrap();
        let medium_rtf = results
            .iter()
            .find(|(t, s, _)| *t == ModelSize::Medium && *s == strategy)
            .map(|(_, _, rtf)| *rtf)
            .unwrap();
        assert!(
            tiny_rtf <= medium_rtf,
            "expected Tiny ({tiny_rtf:.3}) to not be slower than Medium ({medium_rtf:.3}) at {strategy:?}"
        );
    }
}
