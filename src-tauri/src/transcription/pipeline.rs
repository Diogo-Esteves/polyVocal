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
    samples_seen: usize,
}

/// A closed speech segment: its raw audio, plus where that audio sits within
/// the recording.
///
/// The offsets are derived from how much audio has been pushed through the
/// pipeline — *not* from whisper's own timestamps, which restart at zero for
/// each buffer it's handed and so can't be compared across segments (which is
/// what SRT export needs).
#[derive(Debug, Clone, PartialEq)]
pub struct ClosedSegment {
    pub samples: Vec<f32>,
    /// Offset of the segment's first sample from the start of the recording.
    pub start_ms: i64,
    /// Offset of the sample just past the segment's last one.
    pub end_ms: i64,
}

/// The pipeline's fixed internal format — capture resamples to 16 kHz mono
/// before frames ever reach it.
const SAMPLE_RATE_HZ: usize = 16_000;

fn samples_to_ms(samples: usize) -> i64 {
    (samples * 1_000 / SAMPLE_RATE_HZ) as i64
}

impl<V: VoiceActivityScorer> RecordingPipeline<V> {
    pub fn new(segmenter: SpeechSegmenter<V>) -> Self {
        Self {
            segmenter,
            session: TranscriptionSession::new(),
            samples_seen: 0,
        }
    }

    /// Feed one frame of 16 kHz mono f32 PCM into the VAD segmenter only —
    /// cheap, safe to call inline on an async task. Returns the closed
    /// segment once enough trailing silence has been seen (or the segment
    /// hit its max length — see `SpeechSegmenter`); transcribing its samples
    /// and reporting the result back via `record_transcript` is the caller's
    /// responsibility.
    pub fn push_frame(&mut self, frame: &[f32]) -> Result<Option<ClosedSegment>> {
        self.samples_seen += frame.len();
        Ok(self
            .segmenter
            .push(frame)?
            .map(|segment| self.close(segment.samples)))
    }

    /// Force-closes an in-progress segment (see `SpeechSegmenter::flush`),
    /// e.g. when the audio stream ends before the usual trailing-silence
    /// hangover closes it naturally.
    pub fn flush(&mut self) -> Option<ClosedSegment> {
        self.segmenter
            .flush()
            .map(|segment| self.close(segment.samples))
    }

    /// A segment's buffered samples are always the contiguous run of frames
    /// ending at the one that closed it (the segmenter drops silence only
    /// *before* a segment starts), so the segment's span is simply the last
    /// `samples.len()` samples of everything pushed so far.
    fn close(&self, samples: Vec<f32>) -> ClosedSegment {
        let end = self.samples_seen;
        let start = end.saturating_sub(samples.len());
        ClosedSegment {
            samples,
            start_ms: samples_to_ms(start),
            end_ms: samples_to_ms(end),
        }
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
            if let Some(segment) = pipeline.push_frame(&frame).unwrap() {
                produced = Some(segment.samples);
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

        let segment = pipeline.flush().expect("in-progress speech should flush");
        assert_eq!(segment.samples.len(), 12);
    }

    /// Segment offsets must be positions within the *recording*, so
    /// consecutive segments don't overlap (whisper's own timestamps restart
    /// at zero per buffer — see `ClosedSegment`).
    #[test]
    fn test_closed_segments_carry_session_relative_offsets() {
        // Two segments, each 1600 speech samples + hangover, separated by
        // leading silence that belongs to neither.
        let scores = vec![0.9, 0.0, 0.0, /* silence gap */ 0.0, 0.9, 0.0, 0.0];
        let mut pipeline = RecordingPipeline::new(segmenter(scores));

        // 1600 samples per frame = 100ms at 16kHz, for readable assertions.
        let frame = vec![0.1f32; 1600];
        let mut closed = Vec::new();
        for _ in 0..7 {
            if let Some(segment) = pipeline.push_frame(&frame).unwrap() {
                closed.push(segment);
            }
        }

        assert_eq!(closed.len(), 2, "both segments should have closed");
        // Frames 0..=2 (speech + 2 hangover frames): 0ms -> 300ms.
        assert_eq!(closed[0].start_ms, 0);
        assert_eq!(closed[0].end_ms, 300);
        // Frame 3 is silence before speech and is discarded; frames 4..=6
        // form the second segment: 400ms -> 700ms.
        assert_eq!(closed[1].start_ms, 400);
        assert_eq!(closed[1].end_ms, 700);
    }

    #[test]
    fn test_flushed_segment_carries_session_relative_offsets() {
        let mut pipeline = RecordingPipeline::new(segmenter(vec![0.0, 0.9, 0.9]));
        let frame = vec![0.1f32; 1600];
        for _ in 0..3 {
            pipeline.push_frame(&frame).unwrap();
        }

        let segment = pipeline.flush().expect("in-progress speech should flush");
        // Leading silent frame discarded; speech runs 100ms -> 300ms.
        assert_eq!(segment.start_ms, 100);
        assert_eq!(segment.end_ms, 300);
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
