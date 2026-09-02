use crate::audio::capture::{AudioCapture, CaptureCounters};
use crate::audio::chunker::FrameChunker;
use crate::audio::device;
use crate::audio::state::AudioState;
use crate::models::manager::ModelManager;
use crate::models::registry::{ModelSize, VadModel};
use crate::storage::models::TranscriptSegment;
use crate::storage::repository::SessionRepository;
use crate::transcription::calibration::{self, CalibrationResult};
use crate::transcription::engine::{
    DecodeOptions, DecodeStrategy, TranscriptResult, TranscriptionEngine,
};
use crate::transcription::pipeline::{ClosedSegment, RecordingPipeline};
use crate::transcription::session::TranscriptionSession;
use crate::vad::segmenter::SpeechSegmenter;
use crate::vad::silero::{SileroVad, SILERO_FRAME_SIZE};
use crate::vad::{VAD_MAX_SEGMENT_FRAMES, VAD_MIN_SILENCE_FRAMES, VAD_THRESHOLD};
use serde::Serialize;
use sqlx::SqlitePool;
use std::future::Future;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;

/// A recording in progress.
///
/// `AudioCapture` itself is `!Send` (its `cpal::Stream` is thread-affine), so
/// it never leaves the dedicated OS thread that builds it — only a stop
/// signal and that thread's `JoinHandle` are kept here, both `Send`.
struct ActiveRecording {
    stop_tx: std::sync::mpsc::Sender<()>,
    capture_thread: std::thread::JoinHandle<()>,
    producer_task: JoinHandle<RecordingStats>,
    worker_task: JoinHandle<TranscriptionSession>,
}

struct RecordingSlot {
    audio_state: AudioState,
    active: Option<ActiveRecording>,
}

impl Default for RecordingSlot {
    fn default() -> Self {
        Self {
            audio_state: AudioState::idle(),
            active: None,
        }
    }
}

/// Tauri-managed slot for the (at most one) active recording.
#[derive(Default)]
pub struct RecordingState(Mutex<RecordingSlot>);

/// Payload for the `transcript:segment` event (DEC-007) — emitted once per
/// VAD-closed speech segment so the frontend can render it live, without
/// polling.
#[derive(Serialize, Clone)]
struct TranscriptSegmentEvent {
    session_id: String,
    text: String,
    language: String,
    start_ms: i64,
    end_ms: i64,
}

/// Payload for the `audio:level` event (#76) — a single smoothed RMS
/// amplitude in `[0, 1]`, emitted at [`LEVEL_EMIT_RATE_HZ`] while recording
/// so the frontend can drive the record button's `--pv-amp` strand meter
/// instead of its fixed idle-loop placeholder.
#[derive(Serialize, Clone)]
struct AudioLevelEvent {
    level: f32,
}

/// Target rate for `audio:level` emissions — the frontend only needs enough
/// of them to read as live motion, and every emission is extra work on the
/// same task that drains the capture channel (issue #45), so this stays
/// deliberately low.
const LEVEL_EMIT_RATE_HZ: u32 = 20;

/// One-pole smoothing factor applied to each chunk's RMS before emission, so
/// the meter reads as one continuous level rather than jittering chunk to
/// chunk.
const LEVEL_SMOOTHING_ALPHA: f32 = 0.3;

/// Bound on the capture→worker segment queue. At the current ~30s VAD
/// segment cap each `ClosedSegment` is ~1.9MB, so this only ever holds a
/// couple of them regardless — a plain bounded `mpsc` channel with
/// `try_send`-drop-on-full is sufficient at this size; see #144.
const SEGMENT_QUEUE_CAPACITY: usize = 2;

/// Outcome of a non-blocking attempt to hand a closed segment to the
/// transcription worker.
#[derive(Debug)]
enum EnqueueOutcome {
    /// Queue was full — the segment was dropped. This is the last-resort
    /// backstop; the worker degrading to greedy decoding when it's already
    /// behind (see `worker_loop`) is the primary defense against ever
    /// reaching this (#144).
    Dropped,
    /// The worker's receiver is gone (task ended/panicked) — the queue is
    /// poisoned; the caller must stop feeding it.
    WorkerGone,
}

fn try_enqueue_segment(
    queue_tx: &mpsc::Sender<ClosedSegment>,
    segment: ClosedSegment,
) -> Result<(), EnqueueOutcome> {
    match queue_tx.try_send(segment) {
        Ok(()) => Ok(()),
        Err(mpsc::error::TrySendError::Full(_)) => Err(EnqueueOutcome::Dropped),
        Err(mpsc::error::TrySendError::Closed(_)) => Err(EnqueueOutcome::WorkerGone),
    }
}

/// Drop/backlog counters surfaced once at session end (#144 item 8) — a
/// single structured `tracing` line, local log only, no telemetry upload
/// (this app's privacy-first design).
struct RecordingStats {
    dropped_capture_chunks: u64,
    dropped_queue_segments: u64,
}

/// Abstraction over "transcribe this audio", so the worker loop's own
/// logic (session accumulation, degrade-before-drop, persistence) can be
/// unit tested without running real whisper-rs inference. `EngineTranscriber`
/// (wrapping `Arc<TranscriptionEngine>`) is the production implementation.
/// `&self`, not `&mut self`: `WhisperContext` is `Send + Sync`, so no borrow
/// needs to be held across the `spawn_blocking` await, unlike the old
/// move-in/move-out `transcribe_segment` this replaces.
trait Transcriber: Send + Sync {
    fn transcribe(
        &self,
        samples: Vec<f32>,
        options: DecodeOptions,
    ) -> impl Future<Output = anyhow::Result<TranscriptResult>> + Send;
}

struct EngineTranscriber {
    engine: Arc<TranscriptionEngine>,
}

impl Transcriber for EngineTranscriber {
    async fn transcribe(
        &self,
        samples: Vec<f32>,
        options: DecodeOptions,
    ) -> anyhow::Result<TranscriptResult> {
        let engine = self.engine.clone();
        tokio::task::spawn_blocking(move || engine.transcribe(&samples, &options))
            .await
            .map_err(|e| anyhow::anyhow!("transcription task panicked: {e}"))?
    }
}

/// Root-mean-square amplitude of a chunk of mono f32 PCM samples, in `[0, 1]`
/// for well-formed audio.
fn rms_level(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// Drains `queue_rx`, transcribing each closed segment and handing the
/// result to `on_segment`, until the producer (the capture-drain loop)
/// closes the queue — i.e. the recording stopped and every already-queued
/// segment (including the final flush) has been delivered; see #144's
/// "drain, don't discard, at shutdown" decision.
///
/// Degrades to greedy decoding for a segment when the queue is already
/// backed up behind it (`queue_rx.len() > 0` right after receiving it) —
/// ~5x cheaper than beam search, strictly better than losing the segment
/// to the producer's drop-on-overload backstop. `default_strategy` is what
/// gets used when *not* behind — startup calibration (#144 Phase 2, in
/// `start_recording`) picks it per-session based on measured RTF on this
/// machine: `Greedy` if even that couldn't keep up within budget, or
/// `BeamSearch` if there was RTF headroom for the better-quality decode.
async fn worker_loop<T, F, Fut>(
    mut queue_rx: mpsc::Receiver<ClosedSegment>,
    transcriber: &T,
    default_strategy: DecodeStrategy,
    mut session: TranscriptionSession,
    mut on_segment: F,
) -> TranscriptionSession
where
    T: Transcriber,
    F: FnMut(&TranscriptResult, i64, i64) -> Fut,
    Fut: Future<Output = ()>,
{
    while let Some(ClosedSegment {
        samples,
        start_ms,
        end_ms,
    }) = queue_rx.recv().await
    {
        let strategy = if !queue_rx.is_empty() {
            DecodeStrategy::Greedy
        } else {
            default_strategy
        };
        let options = DecodeOptions {
            strategy,
            ..DecodeOptions::default()
        };

        match transcriber.transcribe(samples, options).await {
            Ok(result) => {
                session.append(&result.text, &result.language);
                on_segment(&result, start_ms, end_ms).await;
            }
            Err(e) => tracing::error!("transcription pipeline error: {e}"),
        }
    }

    session
}

#[tauri::command]
pub async fn list_input_devices() -> Result<Vec<crate::audio::InputDevice>, String> {
    device::list_input_devices().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    state: State<'_, RecordingState>,
    pool: State<'_, SqlitePool>,
    device_id: Option<String>,
) -> Result<(), String> {
    // Fail fast if a recording is already active — best-effort, since
    // calibration below runs unlocked and two racing calls can both pass
    // this check; the authoritative check-and-commit happens again right
    // after calibration completes, immediately before `slot` is mutated.
    // Without this, a duplicate call would run the entire (up to a minute
    // long) calibration battery before being rejected, instead of failing
    // near-instantly.
    {
        let slot = state.0.lock().await;
        if !slot.audio_state.is_idle() {
            return Err("Already recording".to_string());
        }
    }

    let models_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("models");

    let manager = ModelManager::new(models_dir);
    let whisper_path = manager
        .active_model_path()
        .ok_or_else(|| "No active Whisper model — download one in Settings".to_string())?;
    let starting_tier = ModelSize::ALL_BY_QUALITY
        .into_iter()
        .find(|size| whisper_path.file_name() == Some(std::ffi::OsStr::new(size.filename())))
        .ok_or_else(|| "active Whisper model file name doesn't match any known tier".to_string())?;
    let starting_engine =
        Arc::new(TranscriptionEngine::load(whisper_path).map_err(|e| e.to_string())?);
    let downloaded_tiers: Vec<ModelSize> = manager
        .list()
        .into_iter()
        .filter(|info| info.downloaded)
        .map(|info| info.size)
        .collect();

    // One-shot startup calibration (#144 Phase 2): measure real RTF on this
    // machine at the user's chosen tier (never upgraded past it — that
    // choice is a quality ceiling) and, if it can't keep up live, walk down
    // to a smaller downloaded tier; if it comfortably keeps up, try
    // upgrading decode strategy to beam search instead. Blocking because it
    // runs real Whisper inference; done once, before either task starts, not
    // mid-session. Deliberately run *before* `RecordingState` is locked
    // below — this can take anywhere from a couple seconds to over a minute
    // (every downloaded tier × both strategies, worst case), and holding
    // the lock across it would block `stop_recording` (and any other lock
    // user) for that whole window even though no recording has actually
    // started yet.
    let (engine, default_strategy) = {
        let manager = manager.clone();
        let pcm = calibration::calibration_pcm().map_err(|e| e.to_string())?;
        tokio::task::spawn_blocking(
            move || -> Result<(Arc<TranscriptionEngine>, DecodeStrategy), String> {
                let mut loaded: Vec<(ModelSize, Arc<TranscriptionEngine>)> =
                    vec![(starting_tier, starting_engine)];
                let CalibrationResult {
                    model_size,
                    strategy,
                    samples,
                } = calibration::calibrate(
                    starting_tier,
                    &downloaded_tiers,
                    |tier, decode_strategy| {
                        let engine = match loaded.iter().find(|(size, _)| *size == tier) {
                            Some((_, engine)) => engine.clone(),
                            None => {
                                let engine =
                                    Arc::new(TranscriptionEngine::load(manager.model_path(&tier))?);
                                loaded.push((tier, engine.clone()));
                                engine
                            }
                        };
                        let options = DecodeOptions {
                            strategy: decode_strategy,
                            ..DecodeOptions::default()
                        };
                        let start = std::time::Instant::now();
                        engine.transcribe(&pcm, &options)?;
                        Ok(start.elapsed().as_secs_f64() / calibration::CALIBRATION_AUDIO_SECS)
                    },
                );

                tracing::info!(
                    ?model_size,
                    ?strategy,
                    ?samples,
                    "startup calibration selected transcription tier/strategy"
                );

                let (engine, strategy) = match loaded.iter().find(|(size, _)| *size == model_size) {
                    Some((_, engine)) => (engine.clone(), strategy),
                    None => {
                        // The winning tier's own `TranscriptionEngine::load` failed during
                        // measurement (e.g. a corrupted/partial model file that still passes
                        // `manager.list()`'s `path.exists()` check) — `calibrate()` has no way
                        // to know that and still returned it as the pick. Fall back to
                        // `starting_tier`, whose engine is always loaded up front and never
                        // removed from `loaded`, at Greedy — safer than trusting a strategy
                        // that was measured against a tier which never actually loaded.
                        tracing::warn!(
                            ?model_size,
                            "calibration-selected tier's model failed to load — \
                             falling back to the starting tier at greedy decoding"
                        );
                        let engine = loaded
                            .iter()
                            .find(|(size, _)| *size == starting_tier)
                            .map(|(_, engine)| engine.clone())
                            .expect(
                                "starting_tier's engine is loaded before calibration begins and never removed",
                            );
                        (engine, DecodeStrategy::Greedy)
                    }
                };
                Ok((engine, strategy))
            },
        )
        .await
        .map_err(|e| format!("calibration task panicked: {e}"))??
    };

    let mut slot = state.0.lock().await;

    if !slot.audio_state.is_idle() {
        return Err("Already recording".to_string());
    }

    // Doesn't auto-download — VAD model must already be fetched via Settings,
    // same as the Whisper model above.
    let silero_path = manager.vad_model_path(&VadModel::Silero);
    let scorer = SileroVad::load(silero_path).map_err(|e| e.to_string())?;
    let segmenter = SpeechSegmenter::new(
        scorer,
        VAD_THRESHOLD,
        VAD_MIN_SILENCE_FRAMES,
        VAD_MAX_SEGMENT_FRAMES,
    );
    let pipeline = RecordingPipeline::new(segmenter);
    // `Uuid` is `Copy` — captured below for the emitted events and to seed the
    // worker's `TranscriptionSession` with the same id the frontend already
    // saw, and again after both tasks finish to persist under it.
    let transcription_session_id = uuid::Uuid::new_v4();

    // Build and own the (non-Send) capture stream entirely on a dedicated
    // OS thread; only the frame receiver crosses back to async code.
    let (setup_tx, setup_rx) = tokio::sync::oneshot::channel::<
        Result<(tokio::sync::mpsc::Receiver<Vec<f32>>, CaptureCounters), String>,
    >();
    let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
    let device_for_thread = device_id.clone();

    let capture_thread = std::thread::spawn(move || {
        match AudioCapture::start_blocking(device_for_thread) {
            Ok((capture, rx)) => {
                if setup_tx.send(Ok((rx, capture.counters()))).is_err() {
                    return; // start_recording gave up before we could start
                }
                let _ = stop_rx.recv(); // block this thread until told to stop
                let _ = capture.stop();
            }
            Err(e) => {
                let _ = setup_tx.send(Err(e.to_string()));
            }
        }
    });

    let (rx, capture_counters) = setup_rx
        .await
        .map_err(|_| "audio capture thread died before starting".to_string())??;

    let session_id = transcription_session_id.to_string();

    // The session header row exists from the moment audio starts flowing, so
    // segments have something to attach to as they arrive and an interrupted
    // recording is still recoverable (DEC-009); `stop_recording` finalises
    // it. Created only once capture is confirmed running — a failed start
    // shouldn't leave an empty phantom session in the history.
    begin_session_row(pool.inner(), &session_id, &stop_tx).await?;
    let repository = SessionRepository::new(pool.inner().clone());

    let (queue_tx, queue_rx) = mpsc::channel::<ClosedSegment>(SEGMENT_QUEUE_CAPACITY);

    let producer_task: JoinHandle<RecordingStats> = tokio::spawn({
        let app = app.clone();
        async move {
            let mut pipeline = pipeline;
            let mut chunker = FrameChunker::new(SILERO_FRAME_SIZE);
            let mut rx = rx;
            let mut queue_drops: u64 = 0;
            let mut last_dropped_samples_seen: u64 = 0;

            let mut smoothed_level = 0.0_f32;
            let level_emit_interval =
                std::time::Duration::from_secs_f64(1.0 / LEVEL_EMIT_RATE_HZ as f64);
            let mut last_level_emit = std::time::Instant::now() - level_emit_interval;

            'drain: while let Some(chunk) = rx.recv().await {
                let dropped_samples_now = capture_counters.dropped_samples.load(Ordering::Relaxed);
                let delta = dropped_samples_now.saturating_sub(last_dropped_samples_seen);
                if delta > 0 {
                    pipeline.account_for_dropped_samples(delta as usize);
                    last_dropped_samples_seen = dropped_samples_now;
                }

                smoothed_level += LEVEL_SMOOTHING_ALPHA * (rms_level(&chunk) - smoothed_level);
                if last_level_emit.elapsed() >= level_emit_interval {
                    last_level_emit = std::time::Instant::now();
                    if let Err(e) = app.emit(
                        "audio:level",
                        &AudioLevelEvent {
                            level: smoothed_level,
                        },
                    ) {
                        tracing::error!("failed to emit audio:level: {e}");
                    }
                }

                for frame in chunker.push(&chunk) {
                    match pipeline.push_frame(&frame) {
                        Ok(Some(segment)) => match try_enqueue_segment(&queue_tx, segment) {
                            Ok(()) => {}
                            Err(EnqueueOutcome::Dropped) => {
                                queue_drops += 1;
                                tracing::warn!(
                                    "transcription worker behind — dropped a closed segment"
                                );
                            }
                            Err(EnqueueOutcome::WorkerGone) => {
                                tracing::error!(
                                    "transcription worker task ended — ending recording early"
                                );
                                break 'drain;
                            }
                        },
                        Ok(None) => {}
                        Err(e) => tracing::error!("transcription pipeline error: {e}"),
                    }
                }
            }

            // The capture stream stopped (or the worker died) — recording is
            // ending. If the worker's still alive, flush whatever's left so it
            // isn't silently dropped, and deliver it with a blocking send (not
            // try_send): stop() must reliably hand off the final segment even
            // if the queue happens to be momentarily full (#144's "drain, don't
            // discard, at shutdown" decision — see `worker_loop` for the
            // matching "keep draining until the queue closes" half of it).
            let dropped_samples_now = capture_counters.dropped_samples.load(Ordering::Relaxed);
            let delta = dropped_samples_now.saturating_sub(last_dropped_samples_seen);
            if delta > 0 {
                pipeline.account_for_dropped_samples(delta as usize);
            }
            if let Some(segment) = pipeline.flush() {
                if queue_tx.send(segment).await.is_err() {
                    tracing::error!(
                        "transcription worker task ended before the final segment could be delivered"
                    );
                }
            }

            // Dropping `queue_tx` here (end of scope) closes the queue, which is
            // `worker_loop`'s signal to stop draining and return.
            RecordingStats {
                dropped_capture_chunks: capture_counters.dropped_frames.load(Ordering::Relaxed),
                dropped_queue_segments: queue_drops,
            }
        }
    });

    let worker_task: JoinHandle<TranscriptionSession> = tokio::spawn({
        let app = app.clone();
        let repository = repository.clone();
        let session_id = session_id.clone();
        let session = TranscriptionSession::with_id(transcription_session_id);
        let transcriber = EngineTranscriber {
            engine: engine.clone(),
        };
        async move {
            worker_loop(
                queue_rx,
                &transcriber,
                default_strategy,
                session,
                |result, start_ms, end_ms| {
                    persist_and_emit_segment(
                        &app,
                        &repository,
                        &session_id,
                        result.clone(),
                        start_ms,
                        end_ms,
                    )
                },
            )
            .await
        }
    });

    let device_label = device_id.unwrap_or_else(|| "default".to_string());
    slot.audio_state = slot.audio_state.clone().start_recording(device_label)?;
    slot.active = Some(ActiveRecording {
        stop_tx,
        capture_thread,
        producer_task,
        worker_task,
    });

    Ok(())
}

/// Creates the in-progress session row `start_recording` needs before the
/// transcription task can begin (DEC-009). Split out so the
/// rollback-on-failure path — telling an already-running capture thread to
/// shut down rather than leaking it and the open microphone stream — is
/// unit-testable without a real audio device or Whisper model, the same way
/// `finish_recording` below is tested against a real in-memory SQLite pool
/// rather than a mock.
async fn begin_session_row(
    pool: &SqlitePool,
    session_id: &str,
    stop_tx: &std::sync::mpsc::Sender<()>,
) -> Result<(), String> {
    let repository = SessionRepository::new(pool.clone());
    if let Err(e) = repository.create_in_progress(session_id).await {
        let _ = stop_tx.send(());
        return Err(e.to_string());
    }
    Ok(())
}

/// Writes one transcribed segment to SQLite and emits it to the frontend.
///
/// The write happens as the segment arrives rather than at stop (DEC-009),
/// so a crash mid-recording costs at most the last utterance instead of the
/// whole session. A failed write is logged and swallowed on purpose: losing
/// one segment is far better than aborting a recording the user is still
/// speaking into, and the in-memory transcript still repairs the session's
/// text when `finish_recording` finalises it.
///
/// `start_ms`/`end_ms` come from the VAD pipeline (offsets within the
/// recording), not from whisper's per-buffer timestamps — see `ClosedSegment`.
async fn persist_and_emit_segment(
    app: &AppHandle,
    repository: &SessionRepository,
    session_id: &str,
    result: TranscriptResult,
    start_ms: i64,
    end_ms: i64,
) {
    let segment = TranscriptSegment::new(
        session_id,
        &result.text,
        Some(&result.language),
        start_ms,
        end_ms,
    );

    if let Err(e) = repository.append_segment(&segment).await {
        tracing::error!("failed to persist transcript segment: {e}");
    }

    let payload = TranscriptSegmentEvent {
        session_id: session_id.to_string(),
        text: result.text,
        language: result.language,
        start_ms,
        end_ms,
    };
    if let Err(e) = app.emit("transcript:segment", &payload) {
        tracing::error!("failed to emit transcript:segment: {e}");
    }
}

#[tauri::command]
pub async fn stop_recording(
    state: State<'_, RecordingState>,
    pool: State<'_, SqlitePool>,
) -> Result<String, String> {
    let mut slot = state.0.lock().await;

    if !slot.audio_state.is_recording() {
        return Err("Not recording".to_string());
    }

    let started_at = slot.audio_state.started_at();
    let ActiveRecording {
        stop_tx,
        capture_thread,
        producer_task,
        worker_task,
    } = slot
        .active
        .take()
        .ok_or_else(|| "Not recording".to_string())?;

    slot.audio_state = slot.audio_state.clone().stop_recording()?;

    // Teardown + persistence can fail in several independent ways (a
    // panicked capture thread, a panicked transcription task, a DB write
    // error) — captured here rather than `?`-propagated directly, so a
    // failure can't skip the state reset below and leave the state
    // machine stuck in `Stopping` forever (issue #46).
    let result = finish_recording(
        stop_tx,
        capture_thread,
        producer_task,
        worker_task,
        started_at,
        pool.inner(),
    )
    .await;

    // The capture thread is joined and the transcription task awaited by
    // this point either way — recording really is over, regardless of
    // whether persisting it succeeded — so unconditionally return to
    // Idle rather than routing through `AudioState::finalize()`, which
    // would need its own error handling for what should be unreachable
    // (we hold the only handle to `slot` and just set `Stopping` above).
    slot.audio_state = AudioState::idle();

    result
}

/// Joins the capture thread, awaits the transcription task, and finalises
/// the session row that `start_recording` created — its segments are
/// already on disk (DEC-009), so this only records the total duration and
/// final detected language and flips the status to `complete`. Split out
/// from `stop_recording` so its `Result` can be captured without
/// short-circuiting past the state-machine reset.
async fn finish_recording(
    stop_tx: std::sync::mpsc::Sender<()>,
    capture_thread: std::thread::JoinHandle<()>,
    producer_task: JoinHandle<RecordingStats>,
    worker_task: JoinHandle<TranscriptionSession>,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    pool: &SqlitePool,
) -> Result<String, String> {
    // Signal the capture thread to stop, then join it off the async
    // executor (it blocks on `stop_rx.recv()`/stream teardown).
    let _ = stop_tx.send(());
    tokio::task::spawn_blocking(move || capture_thread.join())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|_| "audio capture thread panicked".to_string())?;

    // Join order matters: the producer must finish (and drop its queue
    // sender) before the worker's queue-drain loop can end naturally — see
    // `worker_loop`.
    let stats = producer_task.await.map_err(|e| e.to_string())?;
    let session = worker_task.await.map_err(|e| e.to_string())?;

    tracing::info!(
        dropped_capture_chunks = stats.dropped_capture_chunks,
        dropped_queue_segments = stats.dropped_queue_segments,
        "recording session ended"
    );

    let duration_ms = started_at
        .map(|t| (chrono::Utc::now() - t).num_milliseconds())
        .unwrap_or(0);

    // The in-memory transcript is authoritative for the final text: it also
    // covers any segment whose incremental write failed (logged, not fatal —
    // see `persist_and_emit_segment`).
    let id = session.id.to_string();
    let repository = SessionRepository::new(pool.clone());
    repository
        .finalise(
            &id,
            &session.transcript,
            session.detected_language.as_deref(),
            duration_ms,
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::{SESSION_STATUS_COMPLETE, SESSION_STATUS_IN_PROGRESS};
    use sqlx::sqlite::SqlitePoolOptions;

    #[test]
    fn rms_level_of_silence_is_zero() {
        assert_eq!(rms_level(&[0.0, 0.0, 0.0]), 0.0);
    }

    #[test]
    fn rms_level_of_empty_chunk_is_zero() {
        assert_eq!(rms_level(&[]), 0.0);
    }

    #[test]
    fn rms_level_of_full_scale_square_wave_is_one() {
        assert!((rms_level(&[1.0, -1.0, 1.0, -1.0]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn rms_level_ignores_sign() {
        let positive = rms_level(&[0.5, 0.5, 0.5]);
        let negative = rms_level(&[-0.5, -0.5, -0.5]);
        assert!((positive - negative).abs() < 1e-6);
        assert!((positive - 0.5).abs() < 1e-6);
    }

    async fn test_pool() -> SqlitePool {
        // A single connection: sqlx pools each connection to a distinct
        // in-memory database, so >1 connection would see empty tables.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        crate::storage::db::run_migrations(&pool)
            .await
            .expect("migrations should run");
        pool
    }

    fn finished_capture_thread() -> std::thread::JoinHandle<()> {
        std::thread::spawn(|| {})
    }

    #[tokio::test]
    async fn finish_recording_succeeds_and_persists_session() {
        let pool = test_pool().await;
        let (stop_tx, _stop_rx) = std::sync::mpsc::channel::<()>();

        let mut session = TranscriptionSession::new();
        session.append("hello world", "en");
        let expected_id = session.id.to_string();

        let producer_task = tokio::spawn(async {
            RecordingStats {
                dropped_capture_chunks: 0,
                dropped_queue_segments: 0,
            }
        });
        let worker_task = tokio::spawn(async move { session });

        let result = finish_recording(
            stop_tx,
            finished_capture_thread(),
            producer_task,
            worker_task,
            None,
            &pool,
        )
        .await;

        let id = result.expect("should succeed");
        assert_eq!(id, expected_id);

        let repository = SessionRepository::new(pool);
        let saved = repository
            .get(&id)
            .await
            .expect("query should succeed")
            .expect("session should be persisted");
        assert_eq!(saved.transcript, "hello world");
        assert_eq!(saved.language.as_deref(), Some("en"));
        assert_eq!(saved.status, SESSION_STATUS_COMPLETE);
    }

    /// The normal path: `start_recording` created an in-progress row and
    /// segments were written to it as they arrived, so finalising must
    /// update that row (not insert a second one) and flip it to `complete`.
    #[tokio::test]
    async fn finish_recording_finalises_the_in_progress_session_created_at_start() {
        let pool = test_pool().await;
        let (stop_tx, _stop_rx) = std::sync::mpsc::channel::<()>();

        let mut session = TranscriptionSession::new();
        session.append("hello world", "en");
        let id = session.id.to_string();

        let repository = SessionRepository::new(pool.clone());
        repository
            .create_in_progress(&id)
            .await
            .expect("in-progress session should be created");
        repository
            .append_segment(&TranscriptSegment::new(
                &id,
                "hello world",
                Some("en"),
                0,
                900,
            ))
            .await
            .expect("segment should persist");

        let producer_task = tokio::spawn(async {
            RecordingStats {
                dropped_capture_chunks: 0,
                dropped_queue_segments: 0,
            }
        });
        let worker_task = tokio::spawn(async move { session });
        let started_at = chrono::Utc::now() - chrono::Duration::milliseconds(1_000);
        finish_recording(
            stop_tx,
            finished_capture_thread(),
            producer_task,
            worker_task,
            Some(started_at),
            &pool,
        )
        .await
        .expect("should succeed");

        let sessions = repository.list(10, 0).await.expect("list should succeed");
        assert_eq!(sessions.len(), 1, "finalising must not insert a second row");
        assert_eq!(sessions[0].id, id);
        assert_eq!(sessions[0].status, SESSION_STATUS_COMPLETE);
        assert!(sessions[0].duration_ms >= 1_000);
        assert_eq!(sessions[0].transcript, "hello world");
        assert_eq!(
            repository
                .segments(&id)
                .await
                .expect("segments query")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn begin_session_row_creates_an_in_progress_row() {
        let pool = test_pool().await;
        let (stop_tx, _stop_rx) = std::sync::mpsc::channel::<()>();
        let id = uuid::Uuid::new_v4().to_string();

        begin_session_row(&pool, &id, &stop_tx)
            .await
            .expect("should succeed");

        let repository = SessionRepository::new(pool);
        let saved = repository
            .get(&id)
            .await
            .expect("query should succeed")
            .expect("session row should exist");
        assert_eq!(saved.status, SESSION_STATUS_IN_PROGRESS);
    }

    /// A failed insert (forced here via a duplicate id, since `id` is the
    /// table's primary key) must not leave the capture thread it's paired
    /// with blocked forever on `stop_rx` — it should be told to shut down.
    #[tokio::test]
    async fn begin_session_row_signals_stop_on_failure() {
        let pool = test_pool().await;
        let id = uuid::Uuid::new_v4().to_string();
        let repository = SessionRepository::new(pool.clone());
        repository
            .create_in_progress(&id)
            .await
            .expect("first insert should succeed");

        let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
        let result = begin_session_row(&pool, &id, &stop_tx).await;

        assert!(result.is_err(), "duplicate id should fail to insert");
        stop_rx
            .try_recv()
            .expect("failure path should signal the capture thread to stop");
    }

    /// Regression test for issue #46: a panicked capture thread must
    /// surface as an `Err`, not panic `stop_recording` itself and leave
    /// the caller unable to reset the state machine back to `Idle`.
    #[tokio::test]
    async fn finish_recording_returns_err_when_capture_thread_panics() {
        let pool = test_pool().await;
        let (stop_tx, _stop_rx) = std::sync::mpsc::channel::<()>();

        let panicked_thread = std::thread::spawn(|| panic!("simulated capture thread panic"));
        let producer_task = tokio::spawn(async {
            RecordingStats {
                dropped_capture_chunks: 0,
                dropped_queue_segments: 0,
            }
        });
        let worker_task = tokio::spawn(async { TranscriptionSession::new() });

        let result = finish_recording(
            stop_tx,
            panicked_thread,
            producer_task,
            worker_task,
            None,
            &pool,
        )
        .await;

        let err = result.expect_err("a panicked capture thread should surface as an error");
        assert!(err.contains("panicked"));
    }

    /// Regression test for issue #46: a panicked transcription task must
    /// surface as an `Err` too, for the same reason.
    #[tokio::test]
    async fn finish_recording_returns_err_when_worker_task_panics() {
        let pool = test_pool().await;
        let (stop_tx, _stop_rx) = std::sync::mpsc::channel::<()>();

        let producer_task = tokio::spawn(async {
            RecordingStats {
                dropped_capture_chunks: 0,
                dropped_queue_segments: 0,
            }
        });
        let worker_task: JoinHandle<TranscriptionSession> =
            tokio::spawn(async { panic!("simulated worker task panic") });

        let result = finish_recording(
            stop_tx,
            finished_capture_thread(),
            producer_task,
            worker_task,
            None,
            &pool,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn finish_recording_returns_err_when_producer_task_panics() {
        let pool = test_pool().await;
        let (stop_tx, _stop_rx) = std::sync::mpsc::channel::<()>();

        let producer_task: JoinHandle<RecordingStats> =
            tokio::spawn(async { panic!("simulated producer task panic") });
        let worker_task = tokio::spawn(async { TranscriptionSession::new() });

        let result = finish_recording(
            stop_tx,
            finished_capture_thread(),
            producer_task,
            worker_task,
            None,
            &pool,
        )
        .await;

        assert!(result.is_err());
    }

    /// Records the `DecodeStrategy` it was called with (so tests can assert
    /// degrade-before-drop actually happened) and returns a scripted result
    /// after an optional artificial delay (so overflow scenarios can be
    /// constructed deterministically).
    struct FakeTranscriber {
        strategies_seen: std::sync::Mutex<Vec<DecodeStrategy>>,
        delay: std::time::Duration,
    }

    impl FakeTranscriber {
        fn new(delay: std::time::Duration) -> Self {
            Self {
                strategies_seen: std::sync::Mutex::new(Vec::new()),
                delay,
            }
        }
    }

    impl Transcriber for FakeTranscriber {
        async fn transcribe(
            &self,
            samples: Vec<f32>,
            options: DecodeOptions,
        ) -> anyhow::Result<TranscriptResult> {
            self.strategies_seen.lock().unwrap().push(options.strategy);
            if !self.delay.is_zero() {
                tokio::time::sleep(self.delay).await;
            }
            Ok(TranscriptResult {
                text: format!("{} samples", samples.len()),
                language: "en".to_string(),
                segments: Vec::new(),
            })
        }
    }

    fn closed_segment(len: usize, start_ms: i64) -> ClosedSegment {
        ClosedSegment {
            samples: vec![0.0f32; len],
            start_ms,
            end_ms: start_ms + len as i64,
        }
    }

    #[test]
    fn try_enqueue_segment_succeeds_while_the_queue_has_room() {
        let (tx, _rx) = mpsc::channel(2);
        assert!(try_enqueue_segment(&tx, closed_segment(4, 0)).is_ok());
    }

    #[test]
    fn try_enqueue_segment_drops_when_the_queue_is_full() {
        let (tx, _rx) = mpsc::channel(2);
        try_enqueue_segment(&tx, closed_segment(4, 0)).unwrap();
        try_enqueue_segment(&tx, closed_segment(4, 100)).unwrap();

        let result = try_enqueue_segment(&tx, closed_segment(4, 200));

        assert!(matches!(result, Err(EnqueueOutcome::Dropped)));
    }

    #[test]
    fn try_enqueue_segment_reports_worker_gone_once_the_receiver_is_dropped() {
        let (tx, rx) = mpsc::channel(2);
        drop(rx);

        let result = try_enqueue_segment(&tx, closed_segment(4, 0));

        assert!(matches!(result, Err(EnqueueOutcome::WorkerGone)));
    }

    #[tokio::test]
    async fn worker_loop_transcribes_every_queued_segment_and_accumulates_the_session() {
        let (tx, rx) = mpsc::channel(4);
        tx.send(closed_segment(4, 0)).await.unwrap();
        tx.send(closed_segment(4, 100)).await.unwrap();
        drop(tx); // closes the queue so worker_loop returns

        let transcriber = FakeTranscriber::new(std::time::Duration::ZERO);
        let mut emitted = Vec::new();
        let session = worker_loop(
            rx,
            &transcriber,
            DecodeStrategy::Greedy,
            TranscriptionSession::new(),
            |result, start_ms, end_ms| {
                emitted.push((result.text.clone(), start_ms, end_ms));
                async {}
            },
        )
        .await;

        assert_eq!(emitted.len(), 2);
        assert!(session.transcript.contains("4 samples"));
    }

    #[tokio::test]
    async fn worker_loop_degrades_to_greedy_when_already_behind_but_not_otherwise() {
        let (tx, rx) = mpsc::channel(4);
        // Two segments queued up front, so when the worker picks up the first
        // one, the second is still waiting behind it (queue_rx.len() > 0).
        tx.send(closed_segment(4, 0)).await.unwrap();
        tx.send(closed_segment(4, 100)).await.unwrap();
        drop(tx);

        let transcriber = FakeTranscriber::new(std::time::Duration::ZERO);
        worker_loop(
            rx,
            &transcriber,
            DecodeStrategy::BeamSearch { beam_size: 5 },
            TranscriptionSession::new(),
            |_, _, _| async {},
        )
        .await;

        let strategies = transcriber.strategies_seen.lock().unwrap().clone();
        assert_eq!(strategies.len(), 2);
        // Behind when picked up (one more still queued) -> degraded to greedy.
        assert_eq!(strategies[0], DecodeStrategy::Greedy);
        // Caught up (nothing left queued) -> the configured default strategy.
        assert_eq!(strategies[1], DecodeStrategy::BeamSearch { beam_size: 5 });
    }

    #[tokio::test]
    async fn worker_loop_producer_can_keep_enqueueing_without_waiting_for_the_worker() {
        // The whole point of decoupling: a slow worker must not block the
        // producer from continuing to hand off segments (up to queue capacity).
        let (tx, rx) = mpsc::channel(SEGMENT_QUEUE_CAPACITY);
        let transcriber = FakeTranscriber::new(std::time::Duration::from_millis(50));

        let worker = tokio::spawn(async move {
            worker_loop(
                rx,
                &transcriber,
                DecodeStrategy::Greedy,
                TranscriptionSession::new(),
                |_, _, _| async {},
            )
            .await
        });

        // Fill the queue without ever needing to await the worker draining it.
        for i in 0..SEGMENT_QUEUE_CAPACITY {
            try_enqueue_segment(&tx, closed_segment(4, i as i64 * 100))
                .expect("queue should have room up to its capacity");
        }
        drop(tx);

        let session = worker.await.unwrap();
        assert!(session.transcript.contains("4 samples"));
    }
}
