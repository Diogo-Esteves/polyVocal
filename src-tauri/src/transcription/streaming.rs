//! Sliding-window / rolling-buffer streaming transcription for partial live
//! results (issue #159/#164).
//!
//! `StreamingWindow` re-transcribes the current in-progress buffer on a
//! periodic tick, emitting committed (stable across two passes) and
//! provisional (unstable tail) text separately. Calls to the output come via
//! a `WindowTranscriber` trait seam so the commit policy can be unit-tested
//! against a scripted fake with no inference at all.

use std::future::Future;

/// Below this, a tick is skipped rather than transcribed — a very short
/// buffer is exactly the condition that makes Whisper hallucinate silence
/// phrases (design note streaming-transcription.md §4). 16 kHz mono, ~1s.
const MIN_TICK_BUFFER_SAMPLES: usize = 16_000;

/// Known Whisper silence/hallucination outputs to suppress from partials.
/// Matched case-insensitively against the trimmed, trailing-punctuation-
/// stripped tick output. Not exhaustive — extend if QA finds more.
const HALLUCINATION_BLOCKLIST: &[&str] = &[
    "thank you",
    "thank you for watching",
    "thanks for watching",
    "subtitles by the amara.org community",
    "please subscribe",
];

/// "Re-transcribe this whole rolling buffer and give me the text." The seam
/// that lets the commit policy be tested without Whisper.
pub trait WindowTranscriber: Send + Sync {
    fn transcribe_window(&self, pcm: &[f32])
        -> impl Future<Output = Result<String, String>> + Send;
}

/// What one tick produced, in the shape a Tauri event would carry.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Tick {
    /// Newly-stable words appended to the committed transcript this tick.
    pub committed: String,
    /// The current unstable tail — everything Whisper currently believes
    /// follows `committed` but that hasn't agreed across two passes yet.
    /// Replaces (not appends to) whatever the previous tick published.
    pub provisional: String,
}

impl Tick {
    pub fn is_noop(&self) -> bool {
        self.committed.is_empty() && self.provisional.is_empty()
    }
}

/// Rolling-buffer re-transcription with a two-pass stable-prefix commit
/// policy, plus a *published* provisional tail (polyVocal renders a
/// transcript panel, not a text cursor, so an unstable tail can simply be
/// re-rendered — no backspace bookkeeping, unlike voxtype's revision mode).
pub struct StreamingWindow<T: WindowTranscriber> {
    pub transcriber: T,
    buffer: Vec<f32>,
    /// Words committed so far (agreed across two consecutive passes).
    committed_words: Vec<String>,
    /// Whisper's word list from the previous tick, for the prefix diff.
    prev_words: Vec<String>,
    /// What was published as provisional last tick, so a tick that changes
    /// nothing can report a no-op instead of re-emitting.
    prev_provisional: String,
    /// How many words past the committed prefix stay provisional rather than
    /// being committed even when stable. 0 = commit everything stable.
    hold_back_words: usize,
}

impl<T: WindowTranscriber> StreamingWindow<T> {
    pub fn new(transcriber: T, hold_back_words: usize) -> Self {
        Self {
            transcriber,
            buffer: Vec::new(),
            committed_words: Vec::new(),
            prev_words: Vec::new(),
            prev_provisional: String::new(),
            hold_back_words,
        }
    }

    pub fn feed(&mut self, samples: &[f32]) {
        self.buffer.extend_from_slice(samples);
    }

    /// End of utterance, without paying for a final inference pass: the
    /// segment-close transcription from the session's own (higher-tier) engine
    /// is what's actually authoritative and gets displayed — this just resets
    /// the window's per-utterance diff state for the next one. Prefer this over
    /// `finalize()` when the caller has no use for the returned text (as
    /// `commands::audio`'s tick loop doesn't — see #181).
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.committed_words.clear();
        self.prev_words.clear();
        self.prev_provisional.clear();
    }

    /// One re-transcription pass over the whole buffer.
    pub async fn tick(&mut self) -> Result<Tick, String> {
        // Skip transcription on buffers shorter than MIN_TICK_BUFFER_SAMPLES to avoid
        // Whisper hallucinations on very short audio.
        if self.buffer.len() < MIN_TICK_BUFFER_SAMPLES {
            return Ok(Tick::default());
        }

        let text = self.transcriber.transcribe_window(&self.buffer).await?;

        // Suppress known hallucination phrases from the blocklist.
        if is_hallucination(&text) {
            return Ok(Tick::default());
        }

        let words: Vec<String> = text.split_whitespace().map(str::to_owned).collect();
        if words.is_empty() {
            return Ok(Tick::default());
        }

        // Stable prefix = the words this pass agrees on with the previous one.
        let stable = common_prefix_len(&self.prev_words, &words);
        // Never un-commit: a later pass that contradicts committed text is
        // ignored below the commit frontier (see `test_regression_below_the_
        // commit_frontier_is_ignored`).
        let frontier = self.committed_words.len();
        let commit_to = stable.saturating_sub(self.hold_back_words).max(frontier);

        let mut committed = String::new();
        if commit_to > frontier && commit_to <= words.len() {
            committed = words[frontier..commit_to].join(" ");
            self.committed_words
                .extend(words[frontier..commit_to].iter().cloned());
        }

        let provisional = words[self.committed_words.len().min(words.len())..].join(" ");

        self.prev_words = words;
        let tick = if provisional == self.prev_provisional && committed.is_empty() {
            Tick::default()
        } else {
            Tick {
                committed,
                provisional: provisional.clone(),
            }
        };
        self.prev_provisional = provisional;
        Ok(tick)
    }

    /// End of utterance: whatever Whisper's last full pass says wins, in
    /// full — the segment-close transcription is authoritative and replaces
    /// every partial published for this utterance.
    pub async fn finalize(&mut self) -> Result<String, String> {
        let text = if self.buffer.is_empty() {
            String::new()
        } else {
            self.transcriber.transcribe_window(&self.buffer).await?
        };
        self.buffer.clear();
        self.committed_words.clear();
        self.prev_words.clear();
        self.prev_provisional.clear();
        Ok(text.trim().to_string())
    }
}

fn word_eq(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase()
    };
    norm(a) == norm(b)
}

fn is_hallucination(text: &str) -> bool {
    let normalized = text.trim().trim_end_matches(['.', '!', '?']).to_lowercase();
    HALLUCINATION_BLOCKLIST
        .iter()
        .any(|&blocked| blocked == normalized)
}

fn common_prefix_len(a: &[String], b: &[String]) -> usize {
    a.iter()
        .zip(b.iter())
        .take_while(|(x, y)| word_eq(x, y))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MutableScriptedTranscriber {
        outputs: Mutex<std::vec::IntoIter<&'static str>>,
    }

    impl WindowTranscriber for MutableScriptedTranscriber {
        async fn transcribe_window(&self, _pcm: &[f32]) -> Result<String, String> {
            let mut outputs = self.outputs.lock().unwrap();
            Ok(outputs
                .next()
                .expect("ran out of scripted whisper outputs")
                .to_string())
        }
    }

    fn scripted_window(
        outputs: Vec<&'static str>,
        hold_back: usize,
    ) -> StreamingWindow<MutableScriptedTranscriber> {
        StreamingWindow::new(
            MutableScriptedTranscriber {
                outputs: Mutex::new(outputs.into_iter()),
            },
            hold_back,
        )
    }

    #[tokio::test]
    async fn test_first_pass_commits_nothing_but_publishes_a_provisional_tail() {
        let mut w = scripted_window(vec!["and so my"], 0);
        w.feed(&[0.1; 16000]);
        let tick = w.tick().await.unwrap();
        assert_eq!(tick.committed, "");
        assert_eq!(tick.provisional, "and so my");
    }

    #[tokio::test]
    async fn test_words_agreed_across_two_passes_are_committed() {
        let mut w = scripted_window(vec!["and so my", "and so my fellow"], 0);
        w.feed(&[0.1; 16000]);
        w.tick().await.unwrap();
        let tick = w.tick().await.unwrap();
        assert_eq!(tick.committed, "and so my");
        assert_eq!(tick.provisional, "fellow");
    }

    #[tokio::test]
    async fn test_committed_text_is_never_re_emitted() {
        let mut w = scripted_window(
            vec![
                "and so my",
                "and so my fellow",
                "and so my fellow Americans",
            ],
            0,
        );
        w.feed(&[0.1; 16000]);
        w.tick().await.unwrap();
        assert_eq!(w.tick().await.unwrap().committed, "and so my");
        let third = w.tick().await.unwrap();
        assert_eq!(third.committed, "fellow");
        assert_eq!(third.provisional, "Americans");
    }

    #[tokio::test]
    async fn test_unstable_tail_is_not_committed_but_is_still_visible() {
        // Whisper flip-flops on the last word; it stays provisional, and the
        // stable prefix in front of it still commits.
        let mut w = scripted_window(vec!["ask not what your", "ask not what you're"], 0);
        w.feed(&[0.1; 16000]);
        w.tick().await.unwrap();
        let tick = w.tick().await.unwrap();
        assert_eq!(tick.committed, "ask not what");
        assert_eq!(tick.provisional, "you're");
    }

    #[tokio::test]
    async fn test_regression_below_the_commit_frontier_is_ignored() {
        // A later pass rewords already-committed text. Committed text is
        // immutable until finalize(), so nothing is un-emitted and the tail is
        // still computed relative to the frontier.
        let mut w = scripted_window(
            vec![
                "ask not what",
                "ask not what your country",
                "ask NOT what your country can",
            ],
            0,
        );
        w.feed(&[0.1; 16000]);
        w.tick().await.unwrap();
        w.tick().await.unwrap();
        let tick = w.tick().await.unwrap();
        assert_eq!(w.committed_words.join(" "), "ask not what your country");
        assert_eq!(tick.provisional, "can");
    }

    #[tokio::test]
    async fn test_hold_back_words_keeps_the_trailing_stable_words_provisional() {
        let mut w = scripted_window(vec!["and so my fellow", "and so my fellow Americans"], 2);
        w.feed(&[0.1; 16000]);
        w.tick().await.unwrap();
        let tick = w.tick().await.unwrap();
        // 4 words stable, 2 held back -> only "and so" commits.
        assert_eq!(tick.committed, "and so");
        assert_eq!(tick.provisional, "my fellow Americans");
    }

    #[tokio::test]
    async fn test_identical_consecutive_passes_produce_a_noop_tick() {
        let mut w = scripted_window(vec!["and so my", "and so my", "and so my"], 0);
        w.feed(&[0.1; 16000]);
        w.tick().await.unwrap();
        assert!(!w.tick().await.unwrap().is_noop()); // commits the prefix
        assert!(w.tick().await.unwrap().is_noop()); // nothing changed
    }

    #[tokio::test]
    async fn test_finalize_returns_the_authoritative_full_text_and_resets() {
        // The third output is finalize's own pass — it re-transcribes the closed
        // buffer and its answer supersedes every partial, including a revision to
        // text already committed ("your" -> "our").
        let mut w = scripted_window(
            vec![
                "ask not what your",
                "ask not what your country",
                "Ask not what our country can do for you.",
            ],
            0,
        );
        w.feed(&[0.1; 16000]); // Feed dummy audio so buffer is not empty
        w.tick().await.unwrap();
        w.tick().await.unwrap();
        assert_eq!(
            w.finalize().await.unwrap(),
            "Ask not what our country can do for you."
        );
        assert!(w.committed_words.is_empty());
        assert!(w.buffer.is_empty());
    }

    #[tokio::test]
    async fn test_tick_sequence_commits_and_publishes_correctly() {
        // A typical multi-pass sequence: first pass publishes provisional,
        // second commits the prefix and updates provisional, third commits
        // more words and then a final identical pass returns a noop. The last
        // output is for finalize(), which runs its own transcribe_window pass.
        let mut w = scripted_window(
            vec![
                "hello world",
                "hello world friend",
                "hello world friend",
                "hello world friend",
                "hello world friend.",
            ],
            0,
        );

        w.feed(&[0.1; 16000]); // Feed dummy audio so buffer is not empty

        // First pass: no prior text to match against, so nothing commits
        let tick1 = w.tick().await.unwrap();
        assert_eq!(tick1.committed, "");
        assert_eq!(tick1.provisional, "hello world");
        assert!(!tick1.is_noop());

        // Second pass: "hello world" now stable across two passes, so it commits
        let tick2 = w.tick().await.unwrap();
        assert_eq!(tick2.committed, "hello world");
        assert_eq!(tick2.provisional, "friend");
        assert!(!tick2.is_noop());

        // Third pass: "friend" is now stable, so it also commits
        let tick3 = w.tick().await.unwrap();
        assert_eq!(tick3.committed, "friend");
        assert_eq!(tick3.provisional, "");
        assert!(!tick3.is_noop());

        // Fourth pass: nothing changed from previous provisional, so noop
        let tick4 = w.tick().await.unwrap();
        assert!(tick4.is_noop());

        // After finalization, all state is cleared
        let final_text = w.finalize().await.unwrap();
        assert_eq!(final_text, "hello world friend.");
        assert!(w.committed_words.is_empty());
        assert!(w.buffer.is_empty());
        assert_eq!(w.prev_provisional, "");
    }

    #[tokio::test]
    async fn test_finalize_on_empty_buffer_skips_transcriber() {
        // If finalize() calls transcribe_window on an empty buffer, this will
        // panic because there are no scripted outputs provided. If the fix is
        // correct, transcribe_window is never called and finalize() returns an
        // empty string without panicking. This scenario occurs when backpressure
        // on the transcription queue prevents feed() from ever being called
        // before a segment closes (see issue #175).
        let mut w = scripted_window(vec![], 0);
        let text = w.finalize().await.unwrap();
        assert_eq!(text, "");
        assert!(w.buffer.is_empty());
        assert!(w.committed_words.is_empty());
        assert!(w.prev_words.is_empty());
        assert_eq!(w.prev_provisional, "");
    }

    #[tokio::test]
    async fn test_tick_on_short_buffer_returns_noop_without_transcribing() {
        // A buffer shorter than MIN_TICK_BUFFER_SAMPLES should skip transcription
        // entirely and return a no-op tick. This is checked by providing no
        // scripted outputs — if transcribe_window is called, this will panic.
        let mut w = scripted_window(vec![], 0);
        w.feed(&[0.1; 1000]); // Feed 1000 samples, much less than MIN_TICK_BUFFER_SAMPLES
        let tick = w.tick().await.unwrap();
        assert!(tick.is_noop());
        assert!(w.buffer.len() < MIN_TICK_BUFFER_SAMPLES);
    }

    #[tokio::test]
    async fn test_tick_suppresses_hallucination_blocklist_phrases() {
        // When the transcriber returns a phrase in the hallucination blocklist
        // (e.g. "Thank you."), tick() should return a no-op and not update
        // prev_words/prev_provisional, so a subsequent real tick isn't corrupted.
        let mut w = scripted_window(vec!["Thank you.", "hello world friend"], 0);
        w.feed(&[0.1; 20000]); // Feed enough samples to pass MIN_TICK_BUFFER_SAMPLES

        // First tick gets a hallucination — should return noop and not commit
        let tick1 = w.tick().await.unwrap();
        assert!(tick1.is_noop());
        assert!(w.committed_words.is_empty());
        assert!(w.prev_words.is_empty());

        // Second tick gets real text — should work normally since prev_words
        // is still empty from the hallucination being skipped
        let tick2 = w.tick().await.unwrap();
        assert_eq!(tick2.provisional, "hello world friend");
        assert!(w.prev_words.contains(&"hello".to_string()));
    }

    #[test]
    fn test_reset_clears_all_per_utterance_state() {
        let mut w = scripted_window(vec!["hello world"], 0);
        w.feed(&[0.1; 16000]);
        w.committed_words = vec!["hello".to_string(), "world".to_string()];
        w.prev_words = vec!["hello".to_string(), "world".to_string()];
        w.prev_provisional = "friend".to_string();

        w.reset();

        assert!(w.buffer.is_empty());
        assert!(w.committed_words.is_empty());
        assert!(w.prev_words.is_empty());
        assert_eq!(w.prev_provisional, "");
    }
}
