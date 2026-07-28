/// Audio capture module.
///
/// Responsible for:
/// - Enumerating input devices via `cpal`
/// - Opening a microphone stream
/// - Feeding raw PCM samples to the transcription pipeline
/// - Applying a ring buffer for chunked inference
pub mod capture;
pub mod device;

pub use capture::AudioCapture;
pub use device::InputDevice;
