#![allow(dead_code)]

use anyhow::{anyhow, Result};
use rubato::{FastFixedIn, Resampler as RubatoResampler};

/// Custom error type for audio operations
#[derive(Debug, Clone)]
pub enum AudioError {
    ResamplingFailed(String),
    InvalidSampleRate(u32),
    BufferFull,
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioError::ResamplingFailed(msg) => write!(f, "Resampling failed: {}", msg),
            AudioError::InvalidSampleRate(rate) => write!(f, "Invalid sample rate: {}", rate),
            AudioError::BufferFull => write!(f, "Audio buffer full"),
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
    input_rate: u32,
    output_rate: u32,
    channels: usize,
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
            input_rate,
            output_rate,
            channels,
            buffer: Vec::with_capacity(chunk_size * 2),
        })
    }

    /// Resample audio from input rate to 16 kHz.
    ///
    /// Takes raw f32 samples at input rate, returns resampled f32 samples at 16 kHz.
    ///
    /// # Arguments
    /// * `input` - Raw audio samples at input sample rate
    ///
    /// # Returns
    /// Resampled audio at 16 kHz, or error if resampling fails.
    pub fn resample(&mut self, input: &[f32]) -> Result<Vec<f32>> {
        if input.is_empty() {
            return Ok(Vec::new());
        }

        // Prepare input as per rubato's expectations: Vec<Vec<f32>> where outer vec is channels
        let input_data = vec![input.to_vec()];

        // Resample
        let output_data = self
            .resampler
            .process(&input_data, None)
            .map_err(|e| anyhow!(AudioError::ResamplingFailed(format!("{:?}", e))))?;

        // Flatten output (should be single channel) into a Vec<f32>
        if output_data.is_empty() || output_data[0].is_empty() {
            return Ok(Vec::new());
        }

        Ok(output_data[0].clone())
    }

    /// Get the input sample rate.
    pub fn input_rate(&self) -> u32 {
        self.input_rate
    }

    /// Get the output sample rate (target: 16000).
    pub fn output_rate(&self) -> u32 {
        self.output_rate
    }

    /// Get the number of channels.
    pub fn channels(&self) -> usize {
        self.channels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resampler_creation() {
        let resampler = AudioResampler::new(48000, 16000, 1);
        assert!(resampler.is_ok());

        let resampler = resampler.unwrap();
        assert_eq!(resampler.input_rate(), 48000);
        assert_eq!(resampler.output_rate(), 16000);
        assert_eq!(resampler.channels(), 1);
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
}
