/// Transcription module.
///
/// Responsible for:
/// - Loading and managing a Whisper model via `whisper-rs`
/// - Running inference on PCM audio chunks
/// - Emitting token-level or segment-level text events to the frontend
/// - Auto-detecting the spoken language
pub mod engine;
pub mod session;

pub use engine::TranscriptionEngine;
pub use session::TranscriptionSession;
