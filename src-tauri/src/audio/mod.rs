/// Audio capture and management module.
///
/// Responsible for:
/// - Enumerating input devices via `cpal`
/// - Resampling audio to 16 kHz mono f32 (whisper.cpp requirement)
/// - Managing recording session state (Idle/Recording/Stopping)
/// - Opening a microphone stream (Phase 2)
/// - Feeding raw PCM samples to the transcription pipeline (Phase 2)
pub mod capture;
pub mod device;
pub mod resampler;
pub mod state;

pub use device::InputDevice;
