#![allow(dead_code)]

use super::engine::TranscriptionEngine;
use super::session::TranscriptionSession;
use crate::vad::segmenter::{SpeechSegmenter, VoiceActivityScorer};
use anyhow::Result;

/// Wires VAD segmentation, transcription, and session accumulation together.
///
/// Feeds one 16 kHz mono f32 frame at a time; whenever the segmenter closes a
/// speech segment, transcribes it and appends the result to the session.
pub struct RecordingPipeline<V: VoiceActivityScorer> {
    segmenter: SpeechSegmenter<V>,
    engine: TranscriptionEngine,
    session: TranscriptionSession,
}

impl<V: VoiceActivityScorer> RecordingPipeline<V> {
    pub fn new(segmenter: SpeechSegmenter<V>, engine: TranscriptionEngine) -> Self {
        Self {
            segmenter,
            engine,
            session: TranscriptionSession::new(),
        }
    }

    /// Feed one frame of 16 kHz mono f32 PCM into the pipeline.
    ///
    /// Returns an error if VAD scoring or transcription fails; otherwise
    /// silently accumulates into the session until a segment closes.
    pub fn push_frame(&mut self, frame: &[f32]) -> Result<()> {
        if let Some(segment) = self.segmenter.push(frame)? {
            let result = self.engine.transcribe(&segment.samples)?;
            self.session.append(&result.text, &result.language);
        }
        Ok(())
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

    /// Loads a stub `TranscriptionEngine` against a throwaway empty file, so
    /// tests don't depend on a real Whisper model being present.
    fn dummy_engine(name: &str) -> TranscriptionEngine {
        let path = std::env::temp_dir().join(name);
        std::fs::write(&path, b"").expect("failed to write dummy model file");
        TranscriptionEngine::load(path).expect("dummy model file should load")
    }

    #[test]
    fn test_silence_does_not_update_session() {
        let scorer = ScriptedScorer::new(vec![0.0; 3]);
        let segmenter = SpeechSegmenter::new(scorer, 0.5, 2);
        let engine = dummy_engine("polyvocal_test_pipeline_silence.bin");
        let mut pipeline = RecordingPipeline::new(segmenter, engine);

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
        let engine = dummy_engine("polyvocal_test_pipeline_segment.bin");
        let mut pipeline = RecordingPipeline::new(segmenter, engine);

        let frame = vec![0.1f32; 4];
        for _ in 0..4 {
            pipeline.push_frame(&frame).unwrap();
        }

        // The stub engine reports "en" for any non-empty audio it receives —
        // this only becomes Some once the segmenter has actually closed a
        // segment and handed it to the engine.
        assert_eq!(pipeline.session().detected_language.as_deref(), Some("en"));
    }

    #[test]
    fn test_finish_returns_accumulated_session() {
        let scorer = ScriptedScorer::new(vec![0.9, 0.9, 0.0, 0.0]);
        let segmenter = SpeechSegmenter::new(scorer, 0.5, 2);
        let engine = dummy_engine("polyvocal_test_pipeline_finish.bin");
        let mut pipeline = RecordingPipeline::new(segmenter, engine);

        let frame = vec![0.1f32; 4];
        for _ in 0..4 {
            pipeline.push_frame(&frame).unwrap();
        }

        let session = pipeline.finish();
        assert_eq!(session.detected_language.as_deref(), Some("en"));
    }
}
