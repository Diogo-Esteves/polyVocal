#![allow(dead_code)]

use super::session::TranscriptionSession;
use crate::vad::segmenter::{SpeechSegmenter, VoiceActivityScorer};
use anyhow::Result;

/// Wires VAD segmentation and session accumulation together. Deliberately
/// does *not* own or call the transcription engine directly (see DEC-006
/// and issue #45) — Whisper inference is CPU-bound and must run off the
/// async executor via `spawn_blocking`, which needs to take ownership of
/// the engine per call; a struct field can't be moved in and out of a
/// `&mut self` method that way. Callers own the engine themselves, call
/// `push_frame`/`flush` to get raw segment samples, transcribe those
/// off-executor on their own, and report the result back via
/// `record_transcript`.
pub struct RecordingPipeline<V: VoiceActivityScorer> {
    segmenter: SpeechSegmenter<V>,
    session: TranscriptionSession,
}

impl<V: VoiceActivityScorer> RecordingPipeline<V> {
    pub fn new(segmenter: SpeechSegmenter<V>) -> Self {
        Self {
            segmenter,
            session: TranscriptionSession::new(),
        }
    }

    /// Feed one frame of 16 kHz mono f32 PCM into the VAD segmenter only —
    /// cheap, safe to call inline on an async task. Returns the closed
    /// segment's raw samples once enough trailing silence has been seen
    /// (or the segment hit its max length — see `SpeechSegmenter`);
    /// transcribing them and reporting the result back via
    /// `record_transcript` is the caller's responsibility.
    pub fn push_frame(&mut self, frame: &[f32]) -> Result<Option<Vec<f32>>> {
        Ok(self.segmenter.push(frame)?.map(|segment| segment.samples))
    }

    /// Force-closes an in-progress segment (see `SpeechSegmenter::flush`),
    /// e.g. when the audio stream ends before the usual trailing-silence
    /// hangover closes it naturally.
    pub fn flush(&mut self) -> Option<Vec<f32>> {
        self.segmenter.flush().map(|segment| segment.samples)
    }

    /// Records a transcription result (for a segment previously returned
    /// by `push_frame` or `flush`) into the accumulated session.
    pub fn record_transcript(&mut self, text: &str, language: &str) {
        self.session.append(text, language);
    }

    /// The session accumulated so far.
    pub fn session(&self) -> &TranscriptionSession {
        &self.session
    }

    /// Consumes the pipeline, returning the finalized session.
    pub fn finish(self) -> TranscriptionSession {
        self.session
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns preset scores in sequence; panics if asked for more than provided.
    struct ScriptedScorer {
        scores: std::vec::IntoIter<f32>,
    }

    impl ScriptedScorer {
        fn new(scores: Vec<f32>) -> Self {
            Self {
                scores: scores.into_iter(),
            }
        }
    }

    impl VoiceActivityScorer for ScriptedScorer {
        fn score(&mut self, _frame: &[f32]) -> Result<f32> {
            Ok(self
                .scores
                .next()
                .expect("scorer ran out of scripted scores"))
        }
    }

    fn segmenter(scores: Vec<f32>) -> SpeechSegmenter<ScriptedScorer> {
        SpeechSegmenter::new(ScriptedScorer::new(scores), 0.5, 2, 100)
    }

    #[test]
    fn test_silence_produces_no_samples() {
        let mut pipeline = RecordingPipeline::new(segmenter(vec![0.0; 3]));

        let frame = vec![0.0f32; 4];
        for _ in 0..3 {
            assert_eq!(pipeline.push_frame(&frame).unwrap(), None);
        }
        assert!(pipeline.session().transcript.is_empty());
    }

    #[test]
    fn test_closed_segment_returns_samples_for_caller_to_transcribe() {
        // 2 speech frames, then 2 silence frames close the segment (hangover = 2).
        let mut pipeline = RecordingPipeline::new(segmenter(vec![0.9, 0.9, 0.0, 0.0]));

        let frame = vec![0.1f32; 4];
        let mut produced = None;
        for _ in 0..4 {
            if let Some(samples) = pipeline.push_frame(&frame).unwrap() {
                produced = Some(samples);
            }
        }

        let samples = produced.expect("segment should close after silence hangover");
        assert_eq!(samples.len(), 16); // 4 frames * 4 samples
                                       // Nothing recorded into the session until the caller reports back.
        assert!(pipeline.session().transcript.is_empty());
    }

    #[test]
    fn test_record_transcript_updates_session() {
        let mut pipeline = RecordingPipeline::new(segmenter(vec![0.9, 0.9, 0.0, 0.0]));
        let frame = vec![0.1f32; 4];
        for _ in 0..4 {
            pipeline.push_frame(&frame).unwrap();
        }

        pipeline.record_transcript("hello", "en");

        assert_eq!(pipeline.session().transcript, "hello");
        assert_eq!(pipeline.session().detected_language.as_deref(), Some("en"));
    }

    #[test]
    fn test_flush_returns_in_progress_samples() {
        let mut pipeline = RecordingPipeline::new(segmenter(vec![0.9, 0.9, 0.9]));
        let frame = vec![0.1f32; 4];
        for _ in 0..3 {
            assert_eq!(pipeline.push_frame(&frame).unwrap(), None);
        }

        let samples = pipeline.flush().expect("in-progress speech should flush");
        assert_eq!(samples.len(), 12);
    }

    #[test]
    fn test_flush_is_none_when_nothing_in_progress() {
        let mut pipeline = RecordingPipeline::new(segmenter(vec![0.0; 2]));
        let frame = vec![0.0f32; 4];
        pipeline.push_frame(&frame).unwrap();
        pipeline.push_frame(&frame).unwrap();

        assert!(pipeline.flush().is_none());
    }

    #[test]
    fn test_finish_returns_accumulated_session() {
        let mut pipeline = RecordingPipeline::new(segmenter(vec![0.9, 0.9, 0.0, 0.0]));
        let frame = vec![0.1f32; 4];
        for _ in 0..4 {
            pipeline.push_frame(&frame).unwrap();
        }
        pipeline.record_transcript("hello", "en");

        let session = pipeline.finish();
        assert_eq!(session.detected_language.as_deref(), Some("en"));
    }
}
