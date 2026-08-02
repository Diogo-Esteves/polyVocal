#![allow(dead_code)]

use anyhow::{anyhow, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use super::resampler::AudioResampler;

/// Convert i16 samples to f32.
fn i16_to_f32(samples: &[i16]) -> Vec<f32> {
    samples.iter().map(|&s| s as f32 / 32768.0).collect()
}

/// Convert u16 samples to f32.
fn u16_to_f32(samples: &[u16]) -> Vec<f32> {
    samples
        .iter()
        .map(|&s| ((s as i32) - 32768) as f32 / 32768.0)
        .collect()
}

/// Handle to an active audio capture session.
pub struct AudioCapture {
    tx: mpsc::Sender<Vec<f32>>,
    _stream: cpal::Stream,
}

impl AudioCapture {
    /// Start capturing audio from the named device (or default if `None`).
    ///
    /// Automatically resamples to 16 kHz mono f32 (whisper.cpp requirement).
    ///
    /// # Arguments
    /// * `device_id` - Optional device name; uses default device if `None`
    ///
    /// # Returns
    /// Tuple of (AudioCapture handle, receiver for resampled frames)
    ///
    /// # Note
    /// `cpal::Stream` is `!Send` (audio callbacks are thread-affine), so
    /// this — and the `AudioCapture` it returns — must stay on whichever
    /// thread calls it for its entire lifetime. This async wrapper exists
    /// only for call-site convenience; see `start_blocking` for driving
    /// capture from a dedicated OS thread instead of an async task.
    pub async fn start(device_id: Option<String>) -> Result<(Self, mpsc::Receiver<Vec<f32>>)> {
        Self::start_blocking(device_id)
    }

    /// Synchronous version of `start`, for callers that need to build the
    /// stream on a specific (non-async) OS thread — e.g. a dedicated audio
    /// thread whose lifetime the caller owns directly.
    pub fn start_blocking(device_id: Option<String>) -> Result<(Self, mpsc::Receiver<Vec<f32>>)> {
        let host = cpal::default_host();

        // Select device
        let device = if let Some(id) = device_id {
            host.input_devices()?
                .find(|d| d.name().ok() == Some(id.clone()))
                .ok_or_else(|| anyhow!("Audio device not found: {}", id))?
        } else {
            host.default_input_device()
                .ok_or_else(|| anyhow!("No default input device available"))?
        };

        // Get device configuration
        let config = device.default_input_config()?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;

        // Create resampler for this device's input rate
        let resampler = Arc::new(Mutex::new(AudioResampler::new(
            sample_rate,
            16000,
            channels,
        )?));

        // Create mpsc channel for resampled frames
        let (tx, rx) = mpsc::channel(64);

        // Build stream based on sample format
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                Self::build_f32_stream(&device, &config, tx.clone(), resampler.clone())?
            }
            cpal::SampleFormat::I16 => {
                Self::build_i16_stream(&device, &config, tx.clone(), resampler.clone())?
            }
            cpal::SampleFormat::U16 => {
                Self::build_u16_stream(&device, &config, tx.clone(), resampler.clone())?
            }
            _ => {
                return Err(anyhow!(
                    "Unsupported sample format: {:?}",
                    config.sample_format()
                ));
            }
        };

        stream.play()?;

        Ok((
            Self {
                tx,
                _stream: stream,
            },
            rx,
        ))
    }

    fn build_f32_stream(
        device: &cpal::Device,
        config: &cpal::SupportedStreamConfig,
        tx: mpsc::Sender<Vec<f32>>,
        resampler: Arc<Mutex<AudioResampler>>,
    ) -> Result<cpal::Stream> {
        let stream_config: cpal::StreamConfig = config.config();
        let stream = device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if let Ok(mut resampler) = resampler.lock() {
                    if let Ok(resampled) = resampler.resample(data) {
                        if !resampled.is_empty() {
                            let _ = tx.try_send(resampled);
                        }
                    }
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        )?;
        Ok(stream)
    }

    fn build_i16_stream(
        device: &cpal::Device,
        config: &cpal::SupportedStreamConfig,
        tx: mpsc::Sender<Vec<f32>>,
        resampler: Arc<Mutex<AudioResampler>>,
    ) -> Result<cpal::Stream> {
        let stream_config: cpal::StreamConfig = config.config();
        let stream = device.build_input_stream(
            &stream_config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                let f32_samples = i16_to_f32(data);
                if let Ok(mut resampler) = resampler.lock() {
                    if let Ok(resampled) = resampler.resample(&f32_samples) {
                        if !resampled.is_empty() {
                            let _ = tx.try_send(resampled);
                        }
                    }
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        )?;
        Ok(stream)
    }

    fn build_u16_stream(
        device: &cpal::Device,
        config: &cpal::SupportedStreamConfig,
        tx: mpsc::Sender<Vec<f32>>,
        resampler: Arc<Mutex<AudioResampler>>,
    ) -> Result<cpal::Stream> {
        let stream_config: cpal::StreamConfig = config.config();
        let stream = device.build_input_stream(
            &stream_config,
            move |data: &[u16], _: &cpal::InputCallbackInfo| {
                let f32_samples = u16_to_f32(data);
                if let Ok(mut resampler) = resampler.lock() {
                    if let Ok(resampled) = resampler.resample(&f32_samples) {
                        if !resampled.is_empty() {
                            let _ = tx.try_send(resampled);
                        }
                    }
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        )?;
        Ok(stream)
    }

    /// Stop capturing and close the stream.
    ///
    /// The stream is automatically stopped when `AudioCapture` is dropped.
    pub fn stop(self) -> Result<()> {
        drop(self._stream);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_i16_to_f32_conversion() {
        let i16_samples = vec![0i16, 16384, 32767, -32768];
        let f32_samples = i16_to_f32(&i16_samples);

        assert_eq!(f32_samples.len(), 4);
        assert!((f32_samples[0] - 0.0).abs() < 1e-6);
        assert!((f32_samples[1] - 0.5).abs() < 1e-6);
        assert!((f32_samples[2] - 0.9999).abs() < 1e-3);
        assert!(f32_samples[3] < -0.9999);
    }

    #[test]
    fn test_u16_to_f32_conversion() {
        let u16_samples = vec![32768u16, 49152, 65535, 0];
        let f32_samples = u16_to_f32(&u16_samples);

        assert_eq!(f32_samples.len(), 4);
        assert!((f32_samples[0] - 0.0).abs() < 1e-6);
        assert!((f32_samples[1] - 0.5).abs() < 1e-6);
        assert!(f32_samples[2] > 0.9999);
        assert!(f32_samples[3] < -0.9999);
    }

    #[tokio::test]
    async fn test_capture_invalid_device() {
        let result = AudioCapture::start(Some("nonexistent_device".to_string())).await;
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Audio device not found"));
        }
    }
}
