/// Voice activity detection module.
///
/// Responsible for:
/// - Scoring 16 kHz mono f32 PCM frames for speech probability (Silero via `ort`)
/// - Buffering scored frames into complete speech segments (`SpeechSegmenter`)
pub mod segmenter;
pub mod silero;
