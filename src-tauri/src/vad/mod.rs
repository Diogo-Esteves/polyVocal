/// Voice activity detection module.
///
/// Responsible for:
/// - Scoring 16 kHz mono f32 PCM frames for speech probability (Silero via `ort`)
/// - Buffering scored frames into complete speech segments (`SpeechSegmenter`)
pub mod segmenter;
pub mod silero;

/// Minimum speech-probability score to treat a VAD frame as speech.
pub const VAD_THRESHOLD: f32 = 0.5;
/// Consecutive silent frames required to close a speech segment (~320ms at 32ms/frame).
pub const VAD_MIN_SILENCE_FRAMES: usize = 10;
/// Force-closes a segment after this many frames even without trailing
/// silence — ~30s at 32ms/frame (`silero::SILERO_FRAME_SIZE` = 512 samples
/// @16kHz), matching whisper.cpp's own context window. Bounds memory growth
/// and transcript latency for continuous speech (issue #47).
pub const VAD_MAX_SEGMENT_FRAMES: usize = 938;
