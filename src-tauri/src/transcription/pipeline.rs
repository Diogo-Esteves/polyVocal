#![allow(dead_code)]

use super::engine::{TranscriptResult, TranscriptionEngine};
use super::session::TranscriptionSession;
use crate::vad::segmenter::{SpeechSegmenter, VoiceActivityScorer};
use anyhow::Result;

/// Transcribes a buffer of 16 kHz mono f32 PCM samples.
///
/// Implemented by `TranscriptionEngine` (real whisper-rs inference) in
/// production; tests inject a scripted implementation so `RecordingPipeline`'s
/// wiring (does closing a segment trigger transcription and land in the
/// session?) is verified independently of whether real inference works.
pub trait Transcriber {
    fn transcribe(&self, pcm: &[f32]) -> Result<TranscriptResult>;
}

impl Transcriber for TranscriptionEngine {
    fn transcribe(&self, pcm: &[f32]) -> Result<TranscriptResult> {
        TranscriptionEngine::transcribe(self, pcm)
    }
}

/// Wires VAD segmentation, transcription, and session accumulation together.
///
/// Feeds one 16 kHz mono f32 frame at a time; whenever the segmenter closes a
/// speech segment, transcribes it and appends the result to the session.
pub struct RecordingPipeline<V: VoiceActivityScorer, E: Transcriber> {
    segmenter: SpeechSegmenter<V>,
    engine: E,
    session: TranscriptionSession,
}

impl<V: VoiceActivityScorer, E: Transcriber> RecordingPipeline<V, E> {
    pub fn new(segmenter: SpeechSegmenter<V>, engine: E) -> Self {
        Self {
            segmenter,
            engine,
            session: TranscriptionSession::new(),
        }
    }

    /// Feed one frame of 16 kHz mono f32 PCM into the pipeline.
    ///
    /// Returns an error if VAD scoring or transcription fails. Returns
    /// `Some` (and accumulates into the session) whenever this frame closes
    /// a speech segment — callers use this to push the segment onward (e.g.
    /// as a `transcript:segment` event, per DEC-007) without polling.
    pub fn push_frame(&mut self, frame: &[f32]) -> Result<Option<TranscriptResult>> {
        if let Some(segment) = self.segmenter.push(frame)? {
            let result = self.engine.transcribe(&segment.samples)?;
            self.session.append(&result.text, &result.language);
            return Ok(Some(result));
        }
        Ok(None)
    }

    /// Force-closes and transcribes any in-progress speech segment.
    ///
    /// Callers use this when the audio stream ends before the usual
    /// trailing-silence hangover naturally closes the segment (see
    /// `SpeechSegmenter::flush`) — e.g. recording stops right after the
    /// last word. Without this, that final utterance is silently dropped:
    /// `push_frame` only closes (and transcribes) a segment once enough
    /// trailing silence has actually arrived.
    pub fn flush(&mut self) -> Result<Option<TranscriptResult>> {
        let Some(segment) = self.segmenter.flush() else {
            return Ok(None);
        };
        let result = self.engine.transcribe(&segment.samples)?;
        self.session.append(&result.text, &result.language);
        Ok(Some(result))
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
    use super::super::engine::Segment;
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

    /// Reports a fixed transcript + language for any non-empty audio it
    /// receives, without running real whisper-rs inference.
    struct FakeTranscriber;

    impl Transcriber for FakeTranscriber {
        fn transcribe(&self, pcm: &[f32]) -> Result<TranscriptResult> {
            Ok(TranscriptResult {
                text: "hello".to_string(),
                language: "en".to_string(),
                segments: vec![Segment {
                    start_ms: 0,
                    end_ms: (pcm.len() * 1000 / 16000) as i64,
                    text: "hello".to_string(),
                }],
            })
        }
    }

    #[test]
    fn test_silence_does_not_update_session() {
        let scorer = ScriptedScorer::new(vec![0.0; 3]);
        let segmenter = SpeechSegmenter::new(scorer, 0.5, 2);
        let mut pipeline = RecordingPipeline::new(segmenter, FakeTranscriber);

        let frame = vec![0.0f32; 4];
        for _ in 0..3 {
            pipeline.push_frame(&frame).unwrap();
        }

        assert!(pipeline.session().detected_language.is_none());
        assert!(pipeline.session().transcript.is_empty());
    }

    #[test]
    fn test_closed_segment_updates_session() {
        // 2 speech frames, then 2 silence frames close the segment (hangover = 2).
        let scores = vec![0.9, 0.9, 0.0, 0.0];
        let scorer = ScriptedScorer::new(scores);
        let segmenter = SpeechSegmenter::new(scorer, 0.5, 2);
        let mut pipeline = RecordingPipeline::new(segmenter, FakeTranscriber);

        let frame = vec![0.1f32; 4];
        for _ in 0..4 {
            pipeline.push_frame(&frame).unwrap();
        }

        // The fake transcriber reports "en"/"hello" for any non-empty audio
        // it receives — this only becomes Some once the segmenter has
        // actually closed a segment and handed it to the engine.
        assert_eq!(pipeline.session().detected_language.as_deref(), Some("en"));
        assert_eq!(pipeline.session().transcript, "hello");
    }

    #[test]
    fn test_flush_transcribes_in_progress_segment() {
        // Speech throughout, never enough trailing silence to close
        // naturally — mirrors stopping recording right after talking.
        let scorer = ScriptedScorer::new(vec![0.9, 0.9, 0.9]);
        let segmenter = SpeechSegmenter::new(scorer, 0.5, 2);
        let mut pipeline = RecordingPipeline::new(segmenter, FakeTranscriber);

        let frame = vec![0.1f32; 4];
        for _ in 0..3 {
            let result = pipeline.push_frame(&frame).unwrap();
            assert!(result.is_none(), "segment shouldn't close without silence");
        }
        assert!(pipeline.session().transcript.is_empty());

        let flushed = pipeline
            .flush()
            .unwrap()
            .expect("in-progress speech should flush");
        assert_eq!(flushed.text, "hello");
        assert_eq!(pipeline.session().transcript, "hello");
        assert_eq!(pipeline.session().detected_language.as_deref(), Some("en"));
    }

    #[test]
    fn test_flush_is_none_when_nothing_in_progress() {
        let scorer = ScriptedScorer::new(vec![0.0; 2]);
        let segmenter = SpeechSegmenter::new(scorer, 0.5, 2);
        let mut pipeline = RecordingPipeline::new(segmenter, FakeTranscriber);

        let frame = vec![0.0f32; 4];
        pipeline.push_frame(&frame).unwrap();
        pipeline.push_frame(&frame).unwrap();

        assert!(pipeline.flush().unwrap().is_none());
    }

    #[test]
    fn test_finish_returns_accumulated_session() {
        let scorer = ScriptedScorer::new(vec![0.9, 0.9, 0.0, 0.0]);
        let segmenter = SpeechSegmenter::new(scorer, 0.5, 2);
        let mut pipeline = RecordingPipeline::new(segmenter, FakeTranscriber);

        let frame = vec![0.1f32; 4];
        for _ in 0..4 {
            pipeline.push_frame(&frame).unwrap();
        }

        let session = pipeline.finish();
        assert_eq!(session.detected_language.as_deref(), Some("en"));
    }
}
