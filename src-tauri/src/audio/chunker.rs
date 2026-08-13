/// Buffers variable-length audio pushes into fixed-size frames.
///
/// `AudioCapture`'s resampled output isn't guaranteed to land on frame
/// boundaries, but `SpeechSegmenter`/`SileroVad` require exact-size frames.
/// `FrameChunker` sits between the two, carrying leftover samples across
/// calls until enough have accumulated to emit a complete frame.
pub struct FrameChunker {
    frame_size: usize,
    buffer: Vec<f32>,
}

impl FrameChunker {
    /// # Arguments
    /// * `frame_size` - number of samples per emitted frame
    pub fn new(frame_size: usize) -> Self {
        Self {
            frame_size,
            buffer: Vec::new(),
        }
    }

    /// Push new samples in; returns zero or more complete frames.
    ///
    /// Any samples left over after the last complete frame are buffered and
    /// prepended to the next call's input.
    pub fn push(&mut self, samples: &[f32]) -> Vec<Vec<f32>> {
        self.buffer.extend_from_slice(samples);

        let mut frames = Vec::new();
        while self.buffer.len() >= self.frame_size {
            let frame: Vec<f32> = self.buffer.drain(..self.frame_size).collect();
            frames.push(frame);
        }
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_fewer_than_frame_size_buffers() {
        let mut chunker = FrameChunker::new(5);
        let frames = chunker.push(&[1.0, 2.0, 3.0]);
        assert!(frames.is_empty());
    }

    #[test]
    fn test_push_exact_frame_size_returns_one_frame() {
        let mut chunker = FrameChunker::new(5);
        let frames = chunker.push(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert_eq!(frames, vec![vec![1.0, 2.0, 3.0, 4.0, 5.0]]);
    }

    #[test]
    fn test_push_accumulates_across_calls() {
        let mut chunker = FrameChunker::new(5);

        let frames = chunker.push(&[1.0, 2.0, 3.0]);
        assert!(frames.is_empty());

        let frames = chunker.push(&[4.0, 5.0]);
        assert_eq!(frames, vec![vec![1.0, 2.0, 3.0, 4.0, 5.0]]);
    }

    #[test]
    fn test_push_multiple_frames_in_one_call() {
        let mut chunker = FrameChunker::new(5);

        let samples: Vec<f32> = (1..=12).map(|i| i as f32).collect();
        let frames = chunker.push(&samples);

        assert_eq!(
            frames,
            vec![
                vec![1.0, 2.0, 3.0, 4.0, 5.0],
                vec![6.0, 7.0, 8.0, 9.0, 10.0],
            ]
        );
    }

    #[test]
    fn test_remainder_carries_into_next_frame() {
        let mut chunker = FrameChunker::new(5);

        // 12 samples -> 2 frames emitted, 2 samples (11.0, 12.0) buffered.
        let samples: Vec<f32> = (1..=12).map(|i| i as f32).collect();
        chunker.push(&samples);

        // 3 more samples complete the buffered remainder into one frame.
        let frames = chunker.push(&[13.0, 14.0, 15.0]);
        assert_eq!(frames, vec![vec![11.0, 12.0, 13.0, 14.0, 15.0]]);
    }
}
