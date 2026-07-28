use anyhow::Result;
use tokio::sync::mpsc;

/// Handle to an active audio capture session.
pub struct AudioCapture {
    /// Channel sender — push PCM chunks to the transcription pipeline.
    pub tx: mpsc::Sender<Vec<f32>>,
    // TODO: hold the cpal Stream here to keep it alive and allow clean shutdown.
    // _stream: cpal::Stream,
}

impl AudioCapture {
    /// Start capturing audio from the named device (or default if `None`).
    pub async fn start(_device_id: Option<String>) -> Result<(Self, mpsc::Receiver<Vec<f32>>)> {
        let (tx, rx) = mpsc::channel(64);

        // TODO: open cpal input stream, convert samples to f32, send via `tx`
        // Target format: 16 kHz mono f32 (required by whisper.cpp)

        Ok((Self { tx }, rx))
    }

    /// Stop the capture stream gracefully.
    pub async fn stop(self) -> Result<()> {
        // TODO: signal the stream to stop and flush remaining samples
        Ok(())
    }
}
