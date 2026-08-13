use anyhow::{anyhow, Result};
use rubato::{FastFixedIn, Resampler as RubatoResampler};

/// Custom error type for audio operations
#[derive(Debug, Clone)]
pub enum AudioError {
    ResamplingFailed(String),
    InvalidSampleRate(u32),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioError::ResamplingFailed(msg) => write!(f, "Resampling failed: {}", msg),
            AudioError::InvalidSampleRate(rate) => write!(f, "Invalid sample rate: {}", rate),
        }
    }
}

impl std::error::Error for AudioError {}

/// Resampler for converting audio to 16 kHz mono f32 format.
///
/// Wraps rubato::FastFixedIn for cross-platform audio resampling.
/// Target format: 16 kHz, mono, f32.
pub struct AudioResampler {
    resampler: FastFixedIn<f32>,
    buffer: Vec<f32>,
}

impl AudioResampler {
    /// Create a new resampler.
    ///
    /// # Arguments
    /// * `input_rate` - Input sample rate in Hz (e.g., 44100, 48000)
    /// * `output_rate` - Target output rate (typically 16000)
    /// * `channels` - Number of audio channels (typically 1 for mono)
    ///
    /// # Errors
    /// Returns `AudioError` if sample rates are invalid or resampler initialization fails.
    pub fn new(input_rate: u32, output_rate: u32, channels: usize) -> Result<Self> {
        if input_rate == 0 || output_rate == 0 {
            return Err(anyhow!(AudioError::InvalidSampleRate(if input_rate == 0 {
                input_rate
            } else {
                output_rate
            })));
        }

        if channels == 0 {
            return Err(anyhow!("Channels must be > 0"));
        }

        // Rubato requires specifying chunk size; use a reasonable default for 16ms chunks
        let chunk_size = (input_rate as f64 * 0.016) as usize; // 16ms at input rate

        let resampler = FastFixedIn::<f32>::new(
            output_rate as f64 / input_rate as f64,
            2.0,                             // quality factor (1.0-2.0)
            rubato::PolynomialDegree::Cubic, // polynomial degree for interpolation
            chunk_size,
            channels,
        )
        .map_err(|e| anyhow!(AudioError::ResamplingFailed(format!("{:?}", e))))?;

        Ok(Self {
            resampler,
            buffer: Vec::with_capacity(chunk_size * 2),
        })
    }

    /// Resample mono audio from input rate to 16 kHz.
    ///
    /// Takes raw mono f32 samples at input rate, returns resampled f32
    /// samples at 16 kHz. Callers must down-mix multi-channel input to mono
    /// first — `rubato`'s `FastFixedIn` is single-"channel"-vec here by
    /// construction, so feeding it interleaved multi-channel data would
    /// mismatch its configured channel count and fail on every call.
    ///
    /// `rubato`'s fixed-input resampler needs its input in exact
    /// `chunk_size`-frame pieces, but the OS/device decides how many frames
    /// land in each callback (rarely a multiple of `chunk_size`) — so
    /// incoming samples are accumulated in `self.buffer` and only handed to
    /// the resampler once a full chunk is available; any leftover carries
    /// over to the next call instead of being dropped.
    ///
    /// # Arguments
    /// * `input` - Raw mono audio samples at input sample rate
    ///
    /// # Returns
    /// Resampled audio at 16 kHz (possibly empty, if not enough input has
    /// accumulated yet to fill a full chunk), or error if resampling fails.
    pub fn resample(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        self.buffer.extend_from_slice(input);

        let chunk_size = self.resampler.input_frames_next();
        let mut output = Vec::new();

        while self.buffer.len() >= chunk_size {
            let chunk: Vec<f32> = self.buffer.drain(..chunk_size).collect();
            let input_data = vec![chunk];

            let output_data = self
                .resampler
                .process(&input_data, None)
                .map_err(|e| anyhow!(AudioError::ResamplingFailed(format!("{:?}", e))))?;

            if let Some(channel_out) = output_data.into_iter().next() {
                output.extend(channel_out);
            }
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resampler_creation() {
        assert!(AudioResampler::new(48000, 16000, 1).is_ok());
    }

    #[test]
    fn test_invalid_sample_rate() {
        let resampler = AudioResampler::new(0, 16000, 1);
        assert!(resampler.is_err());

        let resampler = AudioResampler::new(48000, 0, 1);
        assert!(resampler.is_err());
    }

    #[test]
    fn test_invalid_channels() {
        let resampler = AudioResampler::new(48000, 16000, 0);
        assert!(resampler.is_err());
    }

    #[test]
    fn test_resample_silence() {
        let mut resampler = AudioResampler::new(48000, 16000, 1).unwrap();

        // Silence: 1000 samples of zeros at 48kHz
        let silence = vec![0.0f32; 1000];
        let output = resampler.resample(&silence).unwrap();

        // Output should be proportionally smaller (48000 -> 16000 is 1:3 ratio)
        assert!(!output.is_empty());
        // Check that silence remains silence
        assert!(output.iter().all(|&s| (s - 0.0).abs() < 1e-5));
    }

    #[test]
    fn test_resample_44khz_to_16khz() {
        let mut resampler = AudioResampler::new(44100, 16000, 1).unwrap();

        // 1 second of silence at 44.1kHz
        let silence = vec![0.0f32; 44100];
        let output = resampler.resample(&silence).unwrap();

        // Rubato buffers internally, so we just verify we get some output
        // The exact count depends on internal buffer sizes
        assert!(!output.is_empty(), "resampler should produce output");
        // Verify output is resampled (should be significantly smaller, ~36% of input)
        assert!(
            output.len() < silence.len(),
            "output should be smaller than input due to rate reduction"
        );
    }

    #[test]
    fn test_resample_96khz_to_16khz() {
        let mut resampler = AudioResampler::new(96000, 16000, 1).unwrap();

        // 1 second of silence at 96kHz
        let silence = vec![0.0f32; 96000];
        let output = resampler.resample(&silence).unwrap();

        // Rubato buffers internally
        assert!(!output.is_empty(), "resampler should produce output");
        // Verify output is resampled (should be ~17% of input, 96kHz -> 16kHz)
        assert!(
            output.len() < silence.len(),
            "output should be smaller than input due to rate reduction"
        );
    }

    #[test]
    fn test_resample_empty_input() {
        let mut resampler = AudioResampler::new(48000, 16000, 1).unwrap();

        let output = resampler.resample(&[]).unwrap();
        assert_eq!(output.len(), 0);
    }

    /// Regression test: real cpal callbacks rarely deliver exactly
    /// `chunk_size` frames — sizes far smaller than a full chunk (as small
    /// as a few dozen samples per callback on some devices) used to error
    /// out of `resample()` entirely (silently swallowed by callers), so no
    /// audio ever reached the VAD/transcription pipeline. Feeding many
    /// small pieces must accumulate across calls and eventually produce
    /// output, not error or silently drop everything.
    #[test]
    fn test_resample_accumulates_small_chunks_without_error() {
        let mut resampler = AudioResampler::new(48000, 16000, 1).unwrap();

        let mut total_output = Vec::new();
        // 100 calls of 50 samples each = 5000 samples, well over one
        // 16ms/48kHz chunk (768 samples), but each individual call is far
        // smaller than a full chunk.
        for _ in 0..100 {
            let small_piece = vec![0.1f32; 50];
            total_output.extend(resampler.resample(&small_piece).unwrap());
        }

        assert!(
            !total_output.is_empty(),
            "accumulated small chunks should eventually produce resampled output"
        );
    }
}
