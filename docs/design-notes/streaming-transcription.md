# Streaming / sliding-window transcription — design note

Exploratory design work for issue **#159** ("Design streaming / sliding-window
transcription for partial live results"). This is a *design note*, not a spec
and not a decision — it exists so a future `implementer`-shaped spec can be
written from it, and so the four open questions in #159 have concrete answers
backed by measurements instead of intuition.

Companion prototype: `src-tauri/tests/streaming_prototype.rs` (throwaway,
self-contained, not wired into the real pipeline).

---

## 1. Where today's latency actually comes from

Current flow (`vad/segmenter.rs` → `transcription/pipeline.rs` →
`commands/audio.rs::worker_loop`):

1. `SpeechSegmenter` buffers 512-sample (32 ms) Silero frames while
   `score >= VAD_THRESHOLD`.
2. It emits a `SpeechSegment` only after `VAD_MIN_SILENCE_FRAMES` (10 frames =
   **320 ms**) of trailing silence, or at `VAD_MAX_SEGMENT_FRAMES` (938 frames
   = **~30 s**).
3. `RecordingPipeline` stamps session-relative offsets, the segment goes onto a
   bounded queue, and `worker_loop` transcribes it off-executor via
   `spawn_blocking`, then `persist_and_emit_segment` writes it to SQLite and
   emits `transcript:segment`.

So the user-visible delay for an utterance of length *L* is:

```
hangover (320 ms)  +  queue wait  +  whisper inference over an L-second buffer
```

The issue text says "several seconds of dead air"; it's worth being precise
about which term dominates, because it changes what a fix has to do. The
hangover is only 320 ms. **The dominant term is the inference call itself**,
and — this is the load-bearing fact for the whole design — whisper.cpp pads
every buffer to a fixed 30 s encoder window regardless of how much audio is in
it (already noted in the header of `src-tauri/tests/rtf_benchmark.rs`). A 3 s
utterance costs nearly as much encoder time as a 25 s one.

That has two consequences:

- Today's latency is roughly **flat in utterance length** and set by the active
  tier. Measured directly in §3 (greedy, 8 threads, this dev machine): ~0.6 s
  at Tiny, ~1.2 s at Base, ~4.3 s at Small per call, whatever the utterance
  length — and up to 3× that when whisper's temperature fallback fires. A
  Small-tier user waits ~4-13 s after finishing a sentence before seeing
  anything. *That* is the "dead air", not the 320 ms hangover.

- The #159 premise that "re-transcribing a growing buffer means increasing
  per-call cost" is **mostly false**. Encoder cost is constant; only the
  autoregressive decode scales, and it scales with token count, not seconds.
  Section 3 measures this.

This reframing matters: the sliding window doesn't just add a nice-to-have
partial display, it is the *only* way to get sub-second feedback on any tier
above Tiny, because the final pass will always cost a full 30 s-window
inference.

---

## 2. Prior art: voxtype's `sliding_window.rs`

Read at `peteonrails/voxtype@dev:src/transcribe/sliding_window.rs` (~1650
lines, itself ported from `nova-npu`). Shape:

- Rolling `Vec<f32>` buffer, trimmed at `max_buffer_s` (29.0 — the Whisper
  context limit); re-transcribed on a `tokio::time::interval` tick
  (`interval_s` default 0.8 s) inside `spawn_blocking`.
- Gates before inference: `min_audio_s` (1.0 s), whole-buffer RMS floor
  (`MIN_SPEECH_RMS = 0.005`), and a **hallucination blocklist** of the phrases
  Whisper emits when fed silence ("thank you", "thanks for watching",
  "subtitles by the amara.org community", …).
- Two diff modes:
  - **Growing** (buffer hasn't wrapped): commit `common_prefix_len(prev_words,
    curr_words)` beyond `confirmed_words`.
  - **Sliding** (buffer wrapped, prefix audio dropped): diff the output against
    the last N emitted words, then commit only the common prefix of *this*
    tick's delta with the *previous* tick's delta.
- Two commit policies:
  - Default **conservative gate** — a word is emitted only once two consecutive
    ticks agree on it. Never revises visible text.
  - Opt-in **revision mode** — types the best-guess tail immediately, and on
    disagreement emits a `Replace { backspace, text }`. `REVISION_LAG_WORDS = 4`
    bounds how far back a correction can reach before a word is frozen.

The two things worth stealing, and the two worth not:

**Steal:** the growing-mode common-prefix stability test (it's simple, and it's
the right primitive), and the frozen-frontier idea (committed text never
changes, bounded revision window).

**Don't steal:**

1. **The backspace machinery.** voxtype types into someone else's text cursor,
   so a correction has to be character-exact and a bookkeeping drift can delete
   text that was never voxtype's. Its own module doc calls this out as the
   riskier failure mode. polyVocal renders a transcript panel it fully owns —
   a provisional tail is just a signal the frontend re-renders. All of
   `Delta::Replace`, `typed_len_from`, `format_typed`, `dedupe_against_emitted`
   collapses to "replace the provisional line".
2. **The whole-tail stability gate.** voxtype's own doc documents the bug: one
   flip-flopping word blocks *everything* after it from committing, observed
   stalling for 18+ seconds. That's a direct consequence of only being able to
   append. polyVocal doesn't need to choose between "wait" and "backspace" —
   it can *show* the unstable tail while withholding it from the committed
   prefix.

Other prior art in the same space, for the record: whisper.cpp's own
`stream` example (fixed-length windows with `--keep` overlap, no stability
diffing at all — visibly churns), and `whisper_streaming` /
"LocalAgreement-2" (Macháček et al., 2023), which is exactly the
two-consecutive-passes-agree rule voxtype implements. LocalAgreement-2 is the
published baseline; we are not inventing a policy here.

---

## 3. Q1 — Compute budget

### What the prototype measures

`src-tauri/tests/streaming_prototype.rs` has two `#[ignore]`d benchmarks:

- `benchmark_growing_buffer_cost` — greedy transcribe wall time at buffer
  lengths 1 → 29 s, per tier. Tests the "cost grows with the utterance"
  premise directly.
- `benchmark_streaming_replay` — a full 1 Hz replay of a 28 s clip through the
  prototype `StreamingWindow`, reporting per-tick inference time, tick-budget
  overruns, and total CPU-seconds spent per second of audio.

Run with:

```bash
cd src-tauri
cargo test --release --test streaming_prototype -- --ignored --nocapture
```

(Release matters: whisper.cpp's C is compiled at the Rust profile's opt-level,
so debug numbers are meaningless.)

### Result 1: per-call cost is essentially flat in buffer length

Greedy, `n_threads = 8`, release build, dev machine (2026-09-07), warm pass
timed after a discarded warm-up pass. `fixtures/jfk.wav` tiled to length.

| Buffer | Tiny | Base | Small |
|---|---|---|---|
| 1 s | 0.60 s | 1.15 s | 3.90 s |
| 2 s | 0.54 s | 1.11 s | 4.24 s |
| 4 s | 0.51 s | 1.14 s | 3.91 s |
| 6 s | 0.57 s | 1.17 s | 4.29 s |
| 8 s | 0.56 s | 1.18 s | 4.20 s |
| 11 s | 0.63 s | 1.22 s | 4.30 s |
| 15 s | 0.87 s | **3.37 s** | **13.03 s** |
| 20 s | 0.66 s | 1.31 s | 4.47 s |
| 25 s | 0.71 s | 1.32 s | 4.55 s |
| 29 s | 1.03 s | 2.13 s | 4.88 s |

**A 29× longer buffer costs 1.7× more at Tiny and 1.9× at Base.** The issue's
"increasing per-call cost as the utterance gets longer" premise is wrong: the
30 s-padded encoder dominates, and the decoder's growth with token count is a
second-order effect. Budgeting can treat a partial pass as **constant cost per
tier**.

The 15 s row is not noise and matters for the design: it is whisper.cpp's
**temperature-fallback retry** firing (the decode trips a compression-ratio or
avg-logprob check and is re-run at a higher temperature), roughly tripling the
call. Re-transcribing a mid-utterance buffer — which by construction ends in
the middle of a phrase — is exactly the condition that provokes this. **Worst-
case tick cost is ~3× the median, not the median**, and a fixed-interval
scheduler will fall behind whenever it fires.

### Result 2: 1 Hz sliding-window replay of a 28 s clip

| Tier | mean tick | max tick | ticks over 1 s | total CPU | cost multiplier |
|---|---|---|---|---|---|
| Tiny | 0.698 s | 1.06 s | 1 / 28 | 19.6 s | **0.7×** |
| Base | 1.574 s | 4.51 s | 28 / 28 | 44.1 s | **1.6×** |
| Small | 4.807 s | 13.65 s | 28 / 28 | 134.6 s | **4.8×** |

"Cost multiplier" = CPU-seconds burned per second of audio, i.e. how much of
one core the partial pass occupies continuously while the user speaks.

**Only Tiny fits a ~1 s tick.** Base sustains 1.6× real time and would never
keep up; Small is off by a factor of four.

Sample of the Tiny trace (`+[...]` = newly committed, `~[...]` = provisional
tail):

```
  t=  1.0s infer= 0.58s  +[]                     ~[And so]
  t=  2.0s infer= 0.51s  +[And so]               ~[my fellow Americans]
  t=  3.0s infer= 0.52s  +[my fellow Americans!] ~[]
  t=  5.0s infer= 0.57s  +[ask]                  ~[not!]
  t=  7.0s infer= 0.68s  +[what]                 ~[your country can do.]
  t=  8.0s infer= 0.58s  +[your country can do]  ~[for you.]
```

Words become **visible** within ~1 tick of being spoken and **committed**
within ~2 ticks — versus ~15 s of nothing at all on today's Small path. That
is the whole case for the feature, and it holds.

Two honest caveats on this replay:

- The 28 s clip is `jfk.wav` tiled ~2.5×, so it contains literal repetition.
  At `t=13 s` Tiny's provisional tail briefly ballooned into a 4× repeated
  sentence (a classic Whisper repetition loop). Real speech won't repeat like
  that, but it does mean **the UI must cap how much provisional tail it
  renders** — an unbounded one can visually explode.
- Committed text here is per-word-correct but punctuation-noisy ("Americans!"
  vs "Americans,"), which is exactly what §4's "segment close replaces the
  whole line" rule is for.



### Recommendation

**Give streaming its own leaner fast-pass tier — do not reuse the session's
calibrated tier for partials.** The measurements force this: Base and Small
cannot hold a 1 s tick on the fastest machine we have numbers for, so
"re-use the session tier" is not a viable option at all above Tiny.

Rationale beyond the raw numbers:

- The session tier is a *quality* ceiling the user chose (see
  `transcription/calibration.rs` — calibration never upgrades past it). Partials
  are, by construction, provisional and about to be superseded. Spending the
  quality budget on text that will be replaced within seconds is backwards.
- The final segment-close pass at the user's tier still happens and is still
  authoritative, so the fast pass costs nothing in final quality.
- The fast pass runs *concurrently with speech*, i.e. while the machine is also
  running capture + Silero. The existing worker's `degrade-before-drop` logic
  exists precisely because that budget is tight.

Concretely: **partials always run at Tiny (greedy), regardless of session
tier**, on a second `WhisperContext` loaded alongside the session one. Tiny is
~78 MB, is already the auto-download fallback, and is the only tier whose
constant-cost 30 s window fits inside a ~1 s tick on the dev machine.

Two knobs fall out of the measurements:

- **Tick interval is adaptive, not fixed.** Schedule the next tick at
  `max(MIN_TICK, last_infer_time * 1.5)` rather than a fixed 1 Hz `interval`.
  voxtype's fixed interval + `MissedTickBehavior::Skip` degrades into "warn and
  drop audio" when inference overruns; a self-pacing loop degrades into "fewer
  partials", which is strictly the right failure mode here. The
  temperature-fallback spikes measured in §3 (0.6 s → 1.06 s at Tiny, 1.4 s →
  4.5 s at Base) mean a fixed interval *will* be overrun regardless of tier —
  this isn't a hypothetical.
- **Budget from the machine, not from a constant.** Tiny's 0.7× multiplier on
  this machine is comfortable; on a machine 40% slower it isn't. The existing
  `transcription::calibration` already measures real RTF at session start and
  can measure the fast-pass tier at the same time for a couple of seconds more.
- **Never let the partial pass starve the final pass.** The partial worker must
  hold a lower-priority claim than `worker_loop`: if a `ClosedSegment` is
  waiting, skip the tick.

---

## 4. Q2 — Correction / stability policy

**Recommendation: a three-zone model — committed / provisional / final —
where corrections are *visible but confined to the provisional zone*.**

```
[ committed text (frozen, never changes) ][ provisional tail (may be rewritten each tick) ]
                                  ...at segment close, the whole line is replaced by the
                                     authoritative full-segment transcript.
```

Rules:

1. **Commit rule (LocalAgreement-2).** A word joins the committed prefix when
   two consecutive passes agree on it, compared case- and
   trailing-punctuation-insensitively (`word_eq` in the prototype). This is the
   published baseline and what voxtype uses.
2. **Committed text is immutable during the utterance.** A later pass that
   contradicts it is ignored below the frontier — no un-emitting, no
   flickering of settled text. (Prototype:
   `test_regression_below_the_commit_frontier_is_ignored`.)
3. **The unstable tail is still shown**, visually distinguished (dimmed /
   italic / lower opacity, matching the existing `.transcript` styling), and is
   *replaced wholesale* every tick. Never appended to.
4. **Segment close is authoritative and may rewrite everything.** When the VAD
   closes the segment, the existing `worker_loop` path transcribes the full
   segment at the session tier and the resulting `transcript:segment` **replaces
   the entire in-progress line**, committed prefix included. This is the single
   correction point where a homophone fix is allowed to touch already-shown
   text, and it coincides with a natural visual beat (end of utterance).

Why not the alternatives:

- *Hold everything until stable, never correct* (voxtype's default) buys
  "never wrong" at the cost of the documented 18-second stall, and buys it for
  a risk polyVocal doesn't carry (no external text cursor).
- *Show everything immediately, correct freely* churns the whole line on every
  tick and is unreadable while someone is talking.
- The three-zone model gets the responsiveness of the latter with the
  readability of the former, because the churn is confined to a visually
  marked ~1-5 word tail.

Supporting parameters:

- `hold_back_words` (prototype knob, suggest default **0**): optionally keep
  the last N stable words provisional anyway. Whisper is most likely to revise
  near the acoustic edge of the buffer; 0 is fine given that the tail is
  visible either way, but the knob is there if the tail proves jumpy in QA.
- **Hallucination and silence gates are mandatory, not optional.** Ticking on a
  short, quiet buffer is exactly the condition that makes Whisper emit "Thank
  you." / "Thanks for watching." — voxtype needed a blocklist for this and so
  will polyVocal. Gate on: buffer ≥ ~1 s, buffer RMS above a floor, and the
  output not matching a small hallucination blocklist. polyVocal has an
  advantage voxtype lacks: **Silero already told us this is speech**, so the
  RMS gate is largely redundant and the window can be VAD-anchored (see §7).

### Persistence

**Partials are never written to SQLite.** `persist_and_emit_segment`'s
crash-safety contract (DEC-009: at most the last utterance is lost) is about
*closed* segments. Partials are UI-only, emitted on a new event and never
touching `TranscriptSegment`. That keeps the storage schema, session
finalisation, and SRT export completely untouched by this feature.

---

## 5. Q3 — Interaction with translation

**Recommendation: never translate partials. No change to the translation path
at all.**

The issue's framing ("live speech is currently translated only after a segment
closes") is actually more generous than reality. Reading
`commands/translation.rs` and `components/session_detail.rs`: translation today
is **whole-session, post-hoc, and explicitly user-initiated** — `translate_text
(session_id, target_lang)` loads a stored session, resolves a source language,
runs OPUS-MT over the full transcript, and persists the result onto the
session. There is no per-segment live translation to extend.

So partial translation is not a small increment on an existing live path; it
would mean building a live translation path that doesn't exist. Beyond that:

- **MT is not incremental.** OPUS-MT translates a sentence; feeding it a
  three-word fragment produces output that is not a prefix of the eventual
  sentence's translation, and reordering languages make this worse (verb-final
  constructions, adjective order). The stable-prefix trick that makes streaming
  ASR work has no equivalent here — there is no "committed prefix" of a
  translation.
- **The cost is in the wrong place.** A partial-translation tick would contend
  with the partial-transcription tick for exactly the CPU budget §3 is already
  fighting for.
- **It solves a problem nobody has.** Translation is a deliberate, one-tap,
  after-the-fact action in this UI.

If live translation is ever wanted, the right unit is a **closed segment**, not
a partial — and that's a separate feature (a live-translate toggle over
`transcript:segment`), orthogonal to #159. This design note explicitly scopes it
out.

---

## 6. Q4 — Opt-in vs. default

**Recommendation: opt-in, off by default, with a Settings toggle
("Show text while I'm still speaking"), for v1.**

Reasons to not default it on:

- It adds a **second loaded Whisper model** (~78 MB resident) and a sustained
  CPU load *during* recording, on top of capture, Silero, and — on Medium/Small
  — a transcription worker that #144 already showed can fall behind. The
  degrade-before-drop machinery exists because this budget is genuinely tight
  on real hardware.
- The measured cost multiplier from `benchmark_streaming_replay` is the number
  to defend this with: a 1 Hz window burns roughly *(mean infer time)* CPU-
  seconds per second of audio, continuously, for the whole utterance.
- It changes the transcript from "text appears, settled" to "text appears and
  moves". That's a real UX regression for short-dictation users, who are the
  ones for whom the feature buys the least (a 3 s utterance barely gets one
  tick in before the final pass lands).
- Privacy-first, local-first apps run on whatever hardware the user has. A
  default that's fine on a 16-core desktop and makes a laptop's fans scream is
  the wrong default.

Reasons it should exist at all: for long-form dictation the current wait is
~15 s on Small and worse on Medium, which is the actual complaint behind #159.

Refinement worth considering once shipped: keep the toggle, but make its
*enablement* calibration-aware — the existing `transcription::calibration`
already measures real RTF at session start, so it can cheaply also measure Tiny
and disable/warn if even the fast pass can't hold a 1 s tick. Reuse
`RTF_KEEP_UP_THRESHOLD`'s existing "leave headroom" reasoning.

---

## 7. Sketch of an implementation shape

Not a spec. The seams that already exist and where new code would sit:

```
capture ──► FrameChunker ──► RecordingPipeline (VAD) ─┬─► ClosedSegment ──► worker_loop
                                                      │      (session tier, authoritative)
                                                      └─► StreamingWindow  (Tiny, greedy)
                                                             every ~1s tick
                                                             └─► transcript:partial event
```

- **`SpeechSegmenter` needs one new accessor, not a rewrite.** The streaming
  window wants the *in-progress* buffer without closing it. Add something like
  `fn in_progress(&self) -> Option<&[f32]>` returning `self.buffer` when
  `in_speech`. This is strictly additive: no existing behaviour changes, and it
  means the streaming window is **VAD-anchored** — it re-transcribes exactly
  the current utterance, never a fixed wall-clock window straddling a pause.
  That's a meaningful simplification over voxtype, which has no VAD and
  therefore needs the whole growing/sliding two-mode split, the RMS gate, and
  the `max_buffer_s` wrap logic. polyVocal's window is bounded by
  `VAD_MAX_SEGMENT_FRAMES` (~30 s) already — **it never wraps, so only
  voxtype's "growing mode" is needed.**
- **A `WindowTranscriber` trait seam** (prototype has it) so the commit policy
  is unit-testable against a scripted fake, exactly like `Translator` in
  `commands/translation.rs` and `Transcriber` in `commands/audio.rs`. The
  policy tests must not need weights.
- **A new Tauri event `transcript:partial`**, payload roughly
  `{ session_id, committed: String, provisional: String }` — or, simpler for
  the frontend, a single cumulative `{ session_id, text, stable_len }`. The
  frontend keeps one "in progress" line, replaces it on each partial, and
  clears it when the matching `transcript:segment` lands.
- **The partial worker is a separate task**, not folded into `worker_loop` —
  `worker_loop` must never block on partial inference, and a failed/slow
  partial pass must degrade to "no partials" without touching the real
  transcript.
- **Reset on every segment close.** The window's diff state is per-utterance;
  `finalize()` clears it (prototype: `test_finalize_returns_the_authoritative_
  full_text_and_resets`).

## 8. Open questions this note does *not* settle

- Exact frontend treatment of the provisional tail (needs a visual QA pass —
  see the repo's note that CSS/appearance changes can't be judged by a
  subagent).
- Whether the final `transcript:segment` replacing the whole line reads as a
  jarring "flash" in practice, or is invisible because most of it is identical.
  Cheap to find out once there's a running build.
- Whether Tiny's partials are *good enough to be useful* for pt/es (the
  `pt_conto.wav` / `es_fabula.wav` fixtures exist and the prototype's replay
  benchmark can be pointed at them).
- Whether a second `WhisperContext` is acceptable memory-wise on the low end,
  or whether the fast pass should share the session context when the session
  tier already *is* Tiny.
