use anyhow::Result;

/// Scores a single frame of 16 kHz mono f32 PCM for speech probability (0.0-1.0).
///
/// Implemented by the Silero/`ort` model in production; tests use a scripted
/// implementation so segmentation logic is verified independently of inference.
pub trait VoiceActivityScorer {
    fn score(&mut self, frame: &[f32]) -> Result<f32>;
}

/// A completed span of speech, ready to hand to the transcription engine.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeechSegment {
    pub samples: Vec<f32>,
}

/// Buffers scored PCM frames into complete speech segments.
///
/// A segment starts on the first frame scoring at/above `threshold` and ends
/// once `min_silence_frames` consecutive frames score below it, or once
/// `max_segment_frames` total frames have been buffered — whichever comes
/// first (issue #47: without a cap, a speaker who never pauses grows the
/// buffer and transcript latency without bound).
pub struct SpeechSegmenter<V: VoiceActivityScorer> {
    scorer: V,
    threshold: f32,
    min_silence_frames: usize,
    max_segment_frames: usize,
    in_speech: bool,
    silence_run: usize,
    frames_in_segment: usize,
    buffer: Vec<f32>,
}

impl<V: VoiceActivityScorer> SpeechSegmenter<V> {
    /// # Arguments
    /// * `scorer` - speech-probability scorer for individual frames
    /// * `threshold` - minimum score to treat a frame as speech
    /// * `min_silence_frames` - consecutive silent frames required to close a segment
    /// * `max_segment_frames` - force-close a segment after this many frames
    ///   total, even without trailing silence
    pub fn new(
        scorer: V,
        threshold: f32,
        min_silence_frames: usize,
        max_segment_frames: usize,
    ) -> Self {
        Self {
            scorer,
            threshold,
            min_silence_frames,
            max_segment_frames,
            in_speech: false,
            silence_run: 0,
            frames_in_segment: 0,
            buffer: Vec::new(),
        }
    }

    /// Feed one frame of 16 kHz mono f32 PCM.
    ///
    /// Returns a completed `SpeechSegment` once enough trailing silence has
    /// been observed after speech, or once the segment hits
    /// `max_segment_frames`; otherwise `None`.
    pub fn push(&mut self, frame: &[f32]) -> Result<Option<SpeechSegment>> {
        let score = self.scorer.score(frame)?;

        if score >= self.threshold {
            self.in_speech = true;
            self.silence_run = 0;
            self.frames_in_segment += 1;
            self.buffer.extend_from_slice(frame);

            if self.frames_in_segment >= self.max_segment_frames {
                return Ok(Some(self.close_segment()));
            }
            return Ok(None);
        }

        if !self.in_speech {
            return Ok(None);
        }

        self.silence_run += 1;
        self.frames_in_segment += 1;
        self.buffer.extend_from_slice(frame);

        if self.silence_run >= self.min_silence_frames
            || self.frames_in_segment >= self.max_segment_frames
        {
            return Ok(Some(self.close_segment()));
        }

        Ok(None)
    }

    /// Force-closes an in-progress speech segment without waiting for the
    /// usual trailing-silence hangover — e.g. when recording is stopped
    /// right after the last word, before `min_silence_frames` of silence
    /// has had a chance to arrive naturally. Returns `None` if no speech
    /// was in progress (plain silence, or nothing pushed since the last
    /// segment closed).
    pub fn flush(&mut self) -> Option<SpeechSegment> {
        if !self.in_speech || self.buffer.is_empty() {
            return None;
        }
        Some(self.close_segment())
    }

    fn close_segment(&mut self) -> SpeechSegment {
        self.in_speech = false;
        self.silence_run = 0;
        self.frames_in_segment = 0;
        let samples = std::mem::take(&mut self.buffer);
        SpeechSegment { samples }
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

    #[test]
    fn test_silence_produces_no_segment() {
        let scorer = ScriptedScorer::new(vec![0.0; 5]);
        let mut segmenter = SpeechSegmenter::new(scorer, 0.5, 2, 100);

        let frame = vec![0.0f32; 10];
        for _ in 0..5 {
            assert_eq!(segmenter.push(&frame).unwrap(), None);
        }
    }

    #[test]
    fn test_speech_then_silence_produces_segment() {
        // 3 speech frames, then 3 silence frames; hangover = 2.
        let scores = vec![0.9, 0.9, 0.9, 0.0, 0.0, 0.0];
        let scorer = ScriptedScorer::new(scores);
        let mut segmenter = SpeechSegmenter::new(scorer, 0.5, 2, 100);

        let frames: Vec<Vec<f32>> = (0..6).map(|i| vec![i as f32; 4]).collect();

        let mut produced = None;
        for frame in &frames {
            if let Some(segment) = segmenter.push(frame).unwrap() {
                produced = Some(segment);
                break;
            }
        }

        let segment = produced.expect("segment should close after silence hangover");
        // 3 speech frames + 2 trailing silence frames (the hangover) = 5 frames * 4 samples.
        assert_eq!(segment.samples.len(), 20);
        // Samples should be the concatenation of frames 0..=4, in order.
        let expected: Vec<f32> = frames[0..5].iter().flatten().copied().collect();
        assert_eq!(segment.samples, expected);
    }

    #[test]
    fn test_silence_before_any_speech_is_discarded() {
        let scores = vec![0.0, 0.0, 0.9, 0.0, 0.0];
        let scorer = ScriptedScorer::new(scores);
        let mut segmenter = SpeechSegmenter::new(scorer, 0.5, 2, 100);

        let frame = vec![1.0f32; 4];

        // Leading silence: no segment, and it must not be retained once speech starts.
        assert_eq!(segmenter.push(&frame).unwrap(), None);
        assert_eq!(segmenter.push(&frame).unwrap(), None);

        // One speech frame, then hangover closes the segment.
        assert_eq!(segmenter.push(&frame).unwrap(), None);
        assert_eq!(segmenter.push(&frame).unwrap(), None);
        let segment = segmenter
            .push(&frame)
            .unwrap()
            .expect("segment should close");

        // Only the speech frame + 2 hangover frames (12 samples) — no leading silence.
        assert_eq!(segment.samples.len(), 12);
    }

    #[test]
    fn test_flush_returns_in_progress_speech() {
        // Speech throughout, never enough trailing silence to close
        // naturally — e.g. the user stops recording right after talking.
        let scores = vec![0.9, 0.9, 0.9];
        let scorer = ScriptedScorer::new(scores);
        let mut segmenter = SpeechSegmenter::new(scorer, 0.5, 2, 100);

        let frame = vec![1.0f32; 4];
        for _ in 0..3 {
            assert_eq!(segmenter.push(&frame).unwrap(), None);
        }

        let flushed = segmenter.flush().expect("in-progress speech should flush");
        assert_eq!(flushed.samples.len(), 12);
    }

    #[test]
    fn test_flush_is_none_when_nothing_in_progress() {
        let scorer = ScriptedScorer::new(vec![0.0; 2]);
        let mut segmenter = SpeechSegmenter::new(scorer, 0.5, 2, 100);

        // Plain silence: nothing ever entered speech.
        let frame = vec![0.0f32; 4];
        segmenter.push(&frame).unwrap();
        segmenter.push(&frame).unwrap();

        assert_eq!(segmenter.flush(), None);
    }

    #[test]
    fn test_flush_after_natural_close_is_none() {
        let scores = vec![0.9, 0.9, 0.0, 0.0];
        let scorer = ScriptedScorer::new(scores);
        let mut segmenter = SpeechSegmenter::new(scorer, 0.5, 2, 100);

        let frame = vec![1.0f32; 4];
        let mut closed = None;
        for _ in 0..4 {
            if let Some(segment) = segmenter.push(&frame).unwrap() {
                closed = Some(segment);
            }
        }
        assert!(closed.is_some(), "segment should have closed naturally");

        // Nothing left in progress after a natural close.
        assert_eq!(segmenter.flush(), None);
    }

    #[test]
    fn test_max_segment_frames_force_closes_without_silence() {
        // Continuous speech, never a single silent frame — only the cap
        // should close this segment.
        let scores = vec![0.9; 5];
        let scorer = ScriptedScorer::new(scores);
        let mut segmenter = SpeechSegmenter::new(scorer, 0.5, 2, 5);

        let frame = vec![1.0f32; 4];
        let mut produced = None;
        for _ in 0..5 {
            if let Some(segment) = segmenter.push(&frame).unwrap() {
                produced = Some(segment);
            }
        }

        let segment = produced.expect("segment should force-close at the frame cap");
        assert_eq!(segment.samples.len(), 20); // 5 frames * 4 samples

        // The segmenter should be ready to start a fresh segment immediately.
        assert_eq!(segmenter.flush(), None);
    }
}
