//! Real-weights benchmarks for issue #159 — sliding-window / rolling-buffer
//! streaming transcription (compute-budget Q1).
//!
//! The core `StreamingWindow` type and its unit tests have been ported to the
//! real library at `src-tauri/src/transcription/streaming.rs`. This file now
//! exists only for the two `#[ignore]`d benchmarks that measure actual hardware
//! RTF using real Whisper weights:
//!
//! - `benchmark_growing_buffer_cost` — transcribe latency vs. buffer length
//! - `benchmark_streaming_replay` — full 1 Hz replay of a ~28 s clip
//!
//! Run with: `cargo test --test streaming_prototype -- --ignored --nocapture`

use polyvocal_lib::transcription::engine::{DecodeOptions, DecodeStrategy, TranscriptionEngine};
use polyvocal_lib::transcription::streaming::WindowTranscriber;
use std::path::PathBuf;

const SAMPLE_RATE: usize = 16_000;

fn fixture_pcm() -> Vec<f32> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/jfk.wav");
    let mut reader = hound::WavReader::open(path).expect("fixture should be a valid WAV file");
    reader
        .samples::<i16>()
        .map(|s| s.expect("valid sample") as f32 / i16::MAX as f32)
        .collect()
}

/// Tiles the ~11 s fixture up to `secs` so a full 30 s window can be exercised.
fn tiled_pcm(secs: f32) -> Vec<f32> {
    let base = fixture_pcm();
    let want = (secs * SAMPLE_RATE as f32) as usize;
    base.iter().cycle().take(want).copied().collect()
}

fn model_path(name: &str) -> Option<PathBuf> {
    let candidates = [
        std::env::temp_dir().join("polyvocal_rtf_benchmark_models"),
        // Hard-coded as well as via `temp_dir()`: a `TMPDIR` override (needed
        // to build this crate on a machine with a small tmpfs) otherwise hides
        // the models `rtf_benchmark` already downloaded.
        PathBuf::from("/tmp/polyvocal_rtf_benchmark_models"),
        dirs_next_data_dir().join("com.polyvocal.app/models"),
    ];
    candidates
        .iter()
        .map(|dir| dir.join(name))
        .find(|p| p.exists())
}

fn dirs_next_data_dir() -> PathBuf {
    std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".into()))
                .join(".local/share")
        })
}

/// Q1 (compute budget): does per-call cost really grow with the buffer?
///
/// whisper.cpp pads every buffer to a fixed 30 s encoder window regardless of
/// how much audio is actually in it, so the encoder is a constant cost and
/// only decoding scales with the number of tokens produced. This measures
/// that directly.
#[test]
#[ignore]
fn benchmark_growing_buffer_cost() {
    let lengths_s = [1.0f32, 2.0, 4.0, 6.0, 8.0, 11.0, 15.0, 20.0, 25.0, 29.0];

    for tier in ["ggml-tiny.bin", "ggml-base.bin", "ggml-small.bin"] {
        let Some(path) = model_path(tier) else {
            eprintln!("SKIP {tier}: not downloaded");
            continue;
        };
        let engine = TranscriptionEngine::load(path).expect("model should load");
        let options = DecodeOptions {
            strategy: DecodeStrategy::Greedy,
            ..DecodeOptions::default()
        };

        eprintln!("\n=== {tier} (greedy) — transcribe wall time vs. buffer length ===");
        for secs in lengths_s {
            let pcm = tiled_pcm(secs);
            // One warm pass, then time the second — first call per model
            // pays page-in costs that a live session wouldn't per tick.
            let _ = engine.transcribe(&pcm, &options);
            let start = std::time::Instant::now();
            let result = engine.transcribe(&pcm, &options).expect("transcribe");
            let elapsed = start.elapsed().as_secs_f64();
            eprintln!(
                "  buffer {secs:>5.1}s -> {elapsed:>6.3}s  ({} words)",
                result.text.split_whitespace().count()
            );
        }
    }
}

/// Q1 + the whole design end-to-end: replay a clip in 1 s ticks through
/// `StreamingWindow` and report when each word first became visible relative
/// to when it was spoken, plus whether the tick loop can keep up.
#[tokio::test]
#[ignore]
async fn benchmark_streaming_replay() {
    use polyvocal_lib::transcription::streaming::StreamingWindow;
    use std::sync::Mutex;

    const TICK_S: f32 = 1.0;

    struct EngineWindowTranscriber {
        engine: TranscriptionEngine,
        options: DecodeOptions,
        infer_times: Mutex<Vec<f64>>,
    }

    impl WindowTranscriber for EngineWindowTranscriber {
        async fn transcribe_window(&self, pcm: &[f32]) -> Result<String, String> {
            let start = std::time::Instant::now();
            // `map_err` to `String`, not `?` into `anyhow`: the dev-dependency
            // graph pulls in a second `anyhow` version, so the two `Error`
            // types aren't convertible here.
            let result = self
                .engine
                .transcribe(pcm, &self.options)
                .map_err(|e| e.to_string())?;
            self.infer_times
                .lock()
                .unwrap()
                .push(start.elapsed().as_secs_f64());
            Ok(result.text)
        }
    }

    for tier in ["ggml-tiny.bin", "ggml-base.bin", "ggml-small.bin"] {
        let Some(path) = model_path(tier) else {
            eprintln!("SKIP {tier}: not downloaded");
            continue;
        };
        let engine = TranscriptionEngine::load(path).expect("model should load");
        let mut window = StreamingWindow::new(
            EngineWindowTranscriber {
                engine,
                options: DecodeOptions {
                    strategy: DecodeStrategy::Greedy,
                    ..DecodeOptions::default()
                },
                infer_times: Mutex::new(Vec::new()),
            },
            0,
        );

        let pcm = tiled_pcm(28.0);
        let chunk = (TICK_S * SAMPLE_RATE as f32) as usize;
        eprintln!("\n=== {tier}: 1 Hz sliding-window replay of 28 s ===");

        let mut audio_t = 0.0f32;
        let mut budget_overruns = 0;
        for block in pcm.chunks(chunk) {
            window.feed(block);
            audio_t += block.len() as f32 / SAMPLE_RATE as f32;
            let tick = window.tick().await.expect("tick");
            let infer = *window
                .transcriber
                .infer_times
                .lock()
                .unwrap()
                .last()
                .unwrap();
            if infer > TICK_S as f64 {
                budget_overruns += 1;
            }
            if !tick.is_noop() {
                eprintln!(
                    "  t={audio_t:>5.1}s infer={infer:>5.2}s  +[{}]  ~[{}]",
                    tick.committed, tick.provisional
                );
            }
        }

        let times = window.transcriber.infer_times.lock().unwrap();
        let total: f64 = times.iter().sum();
        let max = times.iter().cloned().fold(0.0f64, f64::max);
        eprintln!(
            "  ticks={} mean_infer={:.3}s max_infer={:.3}s overruns(>{TICK_S}s)={budget_overruns} \
             total_cpu={total:.1}s for {audio_t:.0}s of audio (cost multiplier {:.1}x)",
            times.len(),
            total / times.len() as f64,
            max,
            total / audio_t as f64,
        );
    }
}
