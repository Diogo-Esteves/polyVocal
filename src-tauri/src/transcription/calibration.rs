//! One-shot startup calibration for live transcription throughput (#144
//! Phase 2).
//!
//! Picks a `(ModelSize, DecodeStrategy)` combination that fits *this*
//! machine's real throughput before a live recording session starts,
//! instead of the fixed `Greedy`-at-whatever-tier-the-user-picked default
//! from Phase 0/1. Deliberately one-shot at session start, not continuous
//! mid-session adaptation from the live lag signal — that's explicitly
//! deferred (see #144).
//!
//! The decision never upgrades past the tier the user selected in
//! Settings — that choice is a quality ceiling, not a suggestion — and
//! only ever downgrades tier (if even greedy decoding can't keep up) or
//! upgrades decoding strategy to `BeamSearch` (if there's RTF headroom at
//! the chosen tier).

use crate::models::registry::ModelSize;
use crate::transcription::engine::DecodeStrategy;

/// Real-time factor: wall-clock transcribe time / audio duration. `1.0`
/// means transcription takes exactly as long as the audio itself; less
/// than `1.0` means it's faster than real time.
pub type Rtf = f64;

/// RTF above this is treated as "can't keep up live", even though `1.0`
/// is the literal keep-up point — the margin covers VAD/capture/other work
/// sharing the machine concurrently with transcription (#144).
pub const RTF_KEEP_UP_THRESHOLD: Rtf = 0.85;

/// One measured `(tier, strategy)` combination, kept for the calibration
/// log line (#144 item 5 — local `tracing` only, no telemetry per DEC-018).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RtfSample {
    pub model_size: ModelSize,
    pub strategy: DecodeStrategy,
    pub rtf: Rtf,
}

/// Outcome of a calibration run: the tier/strategy to use for the session,
/// plus every measurement taken along the way (for the observability log
/// line — see `commands::audio`).
#[derive(Debug, Clone, PartialEq)]
pub struct CalibrationResult {
    pub model_size: ModelSize,
    pub strategy: DecodeStrategy,
    pub samples: Vec<RtfSample>,
}

/// Picks a tier/strategy for the session by walking down from
/// `starting_tier` (the user's Settings selection) through `downloaded`
/// tiers, measuring RTF via `measure` at each step.
///
/// `measure(tier, strategy)` must run (or fake, in tests) a real timed
/// transcription at that tier/strategy and return its RTF; an `Err` is
/// logged and treated as "too slow to keep up" (`Rtf::INFINITY`) so
/// calibration still makes forward progress instead of getting stuck on a
/// single failed measurement.
///
/// Algorithm: at the current tier, measure `Greedy`. If it's within
/// budget, additionally measure `BeamSearch { beam_size: 5 }` — if *that*
/// also fits, prefer it (better quality, same tier); otherwise stay
/// `Greedy` at this tier. If `Greedy` itself doesn't fit budget, step down
/// to the next smaller *downloaded* tier and repeat; once there is no
/// smaller downloaded tier left, accept the smallest tier reached, at
/// `Greedy`, even if it's still over budget — there's nothing smaller to
/// fall back to.
pub fn calibrate<M>(
    starting_tier: ModelSize,
    downloaded: &[ModelSize],
    mut measure: M,
) -> CalibrationResult
where
    M: FnMut(ModelSize, DecodeStrategy) -> anyhow::Result<Rtf>,
{
    let mut samples = Vec::new();
    let mut tier = starting_tier;

    loop {
        let greedy_rtf = match measure(tier, DecodeStrategy::Greedy) {
            Ok(rtf) => rtf,
            Err(e) => {
                tracing::warn!("calibration: greedy measurement failed for {tier:?}: {e}");
                Rtf::INFINITY
            }
        };
        samples.push(RtfSample {
            model_size: tier,
            strategy: DecodeStrategy::Greedy,
            rtf: greedy_rtf,
        });

        if greedy_rtf <= RTF_KEEP_UP_THRESHOLD {
            let beam_strategy = DecodeStrategy::BeamSearch { beam_size: 5 };
            let beam_rtf = match measure(tier, beam_strategy) {
                Ok(rtf) => rtf,
                Err(e) => {
                    tracing::warn!("calibration: beam-search measurement failed for {tier:?}: {e}");
                    Rtf::INFINITY
                }
            };
            samples.push(RtfSample {
                model_size: tier,
                strategy: beam_strategy,
                rtf: beam_rtf,
            });

            let strategy = if beam_rtf <= RTF_KEEP_UP_THRESHOLD {
                beam_strategy
            } else {
                DecodeStrategy::Greedy
            };
            return CalibrationResult {
                model_size: tier,
                strategy,
                samples,
            };
        }

        match tier.next_smaller_downloaded(downloaded) {
            Some(smaller) => tier = smaller,
            None => {
                return CalibrationResult {
                    model_size: tier,
                    strategy: DecodeStrategy::Greedy,
                    samples,
                };
            }
        }
    }
}

/// The same JFK speech fixture the pipeline integration test transcribes
/// (`fixtures/jfk.wav`) is embedded here as calibration audio — real
/// continuous speech, not silence, so the *decoder's* cost (which scales
/// with tokens produced, not just buffer length) is realistic. Silence
/// would make every tier look artificially fast.
static CALIBRATION_SAMPLE_WAV: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/jfk.wav"));

/// Matches the app's own VAD segment cap (`VAD_MAX_SEGMENT_FRAMES` in
/// `vad/mod.rs`) — the longest buffer live transcription ever hands to
/// Whisper. Whisper pads every buffer to a fixed ~30s encoder window
/// regardless of content length, so a shorter calibration clip would give
/// calibration the wrong numbers (#144).
pub const CALIBRATION_AUDIO_SECS: f64 = 30.0;

/// Decodes the embedded calibration fixture and tiles (repeats) it up to
/// [`CALIBRATION_AUDIO_SECS`] of 16kHz mono PCM, for measuring RTF at a
/// realistic buffer size.
pub fn calibration_pcm() -> anyhow::Result<Vec<f32>> {
    let mut reader = hound::WavReader::new(std::io::Cursor::new(CALIBRATION_SAMPLE_WAV))?;
    let spec = reader.spec();
    anyhow::ensure!(
        spec.sample_rate == 16_000 && spec.channels == 1,
        "calibration fixture must be 16kHz mono, got {}Hz/{}ch",
        spec.sample_rate,
        spec.channels
    );
    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.map(|v| v as f32 / 32768.0))
        .collect::<Result<_, _>>()?;
    anyhow::ensure!(
        !samples.is_empty(),
        "calibration fixture decoded to no samples"
    );

    let target_len = (CALIBRATION_AUDIO_SECS * spec.sample_rate as f64) as usize;
    Ok(samples.iter().cycle().take(target_len).copied().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cross-tier sanity check (#144 validation plan item 4) — simulated
    /// fast machine: the starting tier already keeps up easily at greedy,
    /// with enough headroom for beam search too. Calibration must leave it
    /// alone at the upgraded strategy, not downgrade tier.
    #[test]
    fn test_calibrate_upgrades_strategy_on_a_fast_machine_without_downgrading_tier() {
        let downloaded = [ModelSize::Tiny, ModelSize::Small, ModelSize::Medium];
        let result = calibrate(ModelSize::Medium, &downloaded, |_tier, strategy| {
            Ok(match strategy {
                DecodeStrategy::Greedy => 0.2,
                DecodeStrategy::BeamSearch { .. } => 0.6,
            })
        });

        assert_eq!(result.model_size, ModelSize::Medium);
        assert_eq!(result.strategy, DecodeStrategy::BeamSearch { beam_size: 5 });
    }

    /// Cross-tier sanity check, simulated slow machine: the starting tier
    /// can't keep up even at greedy, and only a smaller downloaded tier
    /// does. Calibration must downgrade to that tier.
    #[test]
    fn test_calibrate_downgrades_tier_on_a_slow_machine() {
        let downloaded = [ModelSize::Tiny, ModelSize::Base, ModelSize::Medium];
        let result = calibrate(ModelSize::Medium, &downloaded, |tier, strategy| {
            let rtf = match tier {
                ModelSize::Medium => 2.5,
                ModelSize::Base => 0.7,
                _ => 0.1,
            };
            Ok(match strategy {
                DecodeStrategy::Greedy => rtf,
                DecodeStrategy::BeamSearch { .. } => rtf * 5.0,
            })
        });

        assert_eq!(result.model_size, ModelSize::Base);
        assert_eq!(result.strategy, DecodeStrategy::Greedy);
    }

    #[test]
    fn test_calibrate_never_upgrades_past_the_starting_tier() {
        // Tiny is absurdly fast in this fake, but calibration starts at
        // Small (the user's Settings choice) and must never move up to a
        // larger tier than what it started at, even if a smaller
        // downloaded tier exists that's unused headroom.
        let downloaded = [ModelSize::Tiny, ModelSize::Small, ModelSize::Medium];
        let result = calibrate(ModelSize::Small, &downloaded, |_tier, _strategy| Ok(0.01));

        assert_eq!(result.model_size, ModelSize::Small);
    }

    #[test]
    fn test_calibrate_falls_back_to_smallest_downloaded_tier_when_nothing_keeps_up() {
        let downloaded = [ModelSize::Tiny, ModelSize::Medium];
        let result = calibrate(ModelSize::Medium, &downloaded, |_tier, _strategy| Ok(5.0));

        assert_eq!(result.model_size, ModelSize::Tiny);
        assert_eq!(result.strategy, DecodeStrategy::Greedy);
    }

    #[test]
    fn test_calibrate_stays_at_starting_tier_when_it_is_the_only_downloaded_tier() {
        let downloaded = [ModelSize::Medium];
        let result = calibrate(ModelSize::Medium, &downloaded, |_tier, _strategy| Ok(3.0));

        assert_eq!(result.model_size, ModelSize::Medium);
        assert_eq!(result.strategy, DecodeStrategy::Greedy);
    }

    #[test]
    fn test_calibrate_treats_a_measurement_error_as_too_slow_and_keeps_going() {
        let downloaded = [ModelSize::Tiny, ModelSize::Medium];
        let result = calibrate(ModelSize::Medium, &downloaded, |tier, _strategy| {
            if tier == ModelSize::Medium {
                Err(anyhow::anyhow!("simulated benchmark failure"))
            } else {
                Ok(0.1)
            }
        });

        assert_eq!(result.model_size, ModelSize::Tiny);
    }

    #[test]
    fn test_calibrate_records_every_measurement_taken() {
        let downloaded = [ModelSize::Tiny, ModelSize::Base, ModelSize::Medium];
        let result = calibrate(ModelSize::Medium, &downloaded, |tier, strategy| {
            let rtf = match tier {
                ModelSize::Medium => 2.0,
                ModelSize::Base => 0.5,
                _ => 0.1,
            };
            Ok(match strategy {
                DecodeStrategy::Greedy => rtf,
                DecodeStrategy::BeamSearch { .. } => rtf * 5.0,
            })
        });

        // Medium@Greedy (too slow) -> Base@Greedy (fits) -> Base@BeamSearch (measured for upgrade check).
        assert_eq!(result.samples.len(), 3);
        assert_eq!(result.samples[0].model_size, ModelSize::Medium);
        assert_eq!(result.samples[0].strategy, DecodeStrategy::Greedy);
        assert_eq!(result.samples[1].model_size, ModelSize::Base);
        assert_eq!(result.samples[1].strategy, DecodeStrategy::Greedy);
        assert_eq!(result.samples[2].model_size, ModelSize::Base);
        assert_eq!(
            result.samples[2].strategy,
            DecodeStrategy::BeamSearch { beam_size: 5 }
        );
    }

    #[test]
    fn test_calibration_pcm_is_tiled_to_the_target_duration() {
        let pcm = calibration_pcm().expect("embedded fixture should decode");
        let expected_len = (CALIBRATION_AUDIO_SECS * 16_000.0) as usize;
        assert_eq!(pcm.len(), expected_len);
    }

    #[test]
    fn test_calibration_pcm_is_not_silence() {
        let pcm = calibration_pcm().expect("embedded fixture should decode");
        let rms = (pcm.iter().map(|s| s * s).sum::<f32>() / pcm.len() as f32).sqrt();
        assert!(
            rms > 0.01,
            "calibration audio should contain real speech, rms={rms}"
        );
    }
}
