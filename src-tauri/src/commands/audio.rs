use crate::audio::capture::AudioCapture;
use crate::audio::chunker::FrameChunker;
use crate::audio::device;
use crate::audio::state::AudioState;
use crate::models::manager::ModelManager;
use crate::models::registry::VadModel;
use crate::storage::models::Session;
use crate::storage::repository::SessionRepository;
use crate::transcription::engine::TranscriptionEngine;
use crate::transcription::pipeline::RecordingPipeline;
use crate::transcription::session::TranscriptionSession;
use crate::vad::segmenter::SpeechSegmenter;
use crate::vad::silero::{SileroVad, SILERO_FRAME_SIZE};
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// Minimum speech-probability score to treat a VAD frame as speech.
const VAD_THRESHOLD: f32 = 0.5;
/// Consecutive silent frames required to close a speech segment (~320ms at 32ms/frame).
const VAD_MIN_SILENCE_FRAMES: usize = 10;

/// A recording in progress.
///
/// `AudioCapture` itself is `!Send` (its `cpal::Stream` is thread-affine), so
/// it never leaves the dedicated OS thread that builds it — only a stop
/// signal and that thread's `JoinHandle` are kept here, both `Send`.
struct ActiveRecording {
    stop_tx: std::sync::mpsc::Sender<()>,
    capture_thread: std::thread::JoinHandle<()>,
    task: JoinHandle<TranscriptionSession>,
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

#[tauri::command]
pub async fn list_input_devices() -> Result<Vec<crate::audio::InputDevice>, String> {
    device::list_input_devices().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_recording(
    app: AppHandle,
    state: State<'_, RecordingState>,
    device_id: Option<String>,
) -> Result<(), String> {
    let mut slot = state.0.lock().await;

    if !slot.audio_state.is_idle() {
        return Err("Already recording".to_string());
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
    let engine = TranscriptionEngine::load(whisper_path).map_err(|e| e.to_string())?;

    // Doesn't auto-download — VAD model must already be fetched via Settings,
    // same as the Whisper model above.
    let silero_path = manager.vad_model_path(&VadModel::Silero);
    let scorer = SileroVad::load(silero_path).map_err(|e| e.to_string())?;
    let segmenter = SpeechSegmenter::new(scorer, VAD_THRESHOLD, VAD_MIN_SILENCE_FRAMES);
    let pipeline = RecordingPipeline::new(segmenter, engine);
    // `Uuid` is `Copy`, so capturing it below (for the emitted events) doesn't
    // consume this — it's also used after the task finishes to give the
    // persisted session the same id the frontend saw in live events.
    let transcription_session_id = pipeline.session().id;

    // Build and own the (non-Send) capture stream entirely on a dedicated
    // OS thread; only the frame receiver crosses back to async code.
    let (setup_tx, setup_rx) =
        tokio::sync::oneshot::channel::<Result<tokio::sync::mpsc::Receiver<Vec<f32>>, String>>();
    let (stop_tx, stop_rx) = std::sync::mpsc::channel::<()>();
    let device_for_thread = device_id.clone();

    let capture_thread = std::thread::spawn(move || {
        match AudioCapture::start_blocking(device_for_thread) {
            Ok((capture, rx)) => {
                if setup_tx.send(Ok(rx)).is_err() {
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

    let rx = setup_rx
        .await
        .map_err(|_| "audio capture thread died before starting".to_string())??;

    let task = tokio::spawn(async move {
        let mut pipeline = pipeline;
        let mut chunker = FrameChunker::new(SILERO_FRAME_SIZE);
        let mut rx = rx;

        let emit_segment = |result: crate::transcription::engine::TranscriptResult| {
            let start_ms = result.segments.first().map(|s| s.start_ms).unwrap_or(0);
            let end_ms = result.segments.last().map(|s| s.end_ms).unwrap_or(0);
            let payload = TranscriptSegmentEvent {
                session_id: transcription_session_id.to_string(),
                text: result.text,
                language: result.language,
                start_ms,
                end_ms,
            };
            if let Err(e) = app.emit("transcript:segment", &payload) {
                tracing::error!("failed to emit transcript:segment: {e}");
            }
        };

        while let Some(chunk) = rx.recv().await {
            for frame in chunker.push(&chunk) {
                match pipeline.push_frame(&frame) {
                    Ok(Some(result)) => emit_segment(result),
                    Ok(None) => {}
                    Err(e) => tracing::error!("transcription pipeline error: {e}"),
                }
            }
        }

        // The capture stream stopped — recording was told to stop, likely
        // right after the last word, before the usual trailing-silence
        // hangover had a chance to close the segment naturally. Flush
        // whatever's left so it isn't silently dropped (see
        // RecordingPipeline::flush).
        match pipeline.flush() {
            Ok(Some(result)) => emit_segment(result),
            Ok(None) => {}
            Err(e) => tracing::error!("transcription pipeline error on flush: {e}"),
        }

        pipeline.finish()
    });

    let device_label = device_id.unwrap_or_else(|| "default".to_string());
    slot.audio_state = slot.audio_state.clone().start_recording(device_label)?;
    slot.active = Some(ActiveRecording {
        stop_tx,
        capture_thread,
        task,
    });

    Ok(())
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
        task,
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
    let result = finish_recording(stop_tx, capture_thread, task, started_at, pool.inner()).await;

    // The capture thread is joined and the transcription task awaited by
    // this point either way — recording really is over, regardless of
    // whether persisting it succeeded — so unconditionally return to
    // Idle rather than routing through `AudioState::finalize()`, which
    // would need its own error handling for what should be unreachable
    // (we hold the only handle to `slot` and just set `Stopping` above).
    slot.audio_state = AudioState::idle();

    result
}

/// Joins the capture thread, awaits the transcription task, and persists
/// the resulting session. Split out from `stop_recording` so its `Result`
/// can be captured without short-circuiting past the state-machine reset.
async fn finish_recording(
    stop_tx: std::sync::mpsc::Sender<()>,
    capture_thread: std::thread::JoinHandle<()>,
    task: JoinHandle<TranscriptionSession>,
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

    let session = task.await.map_err(|e| e.to_string())?;

    let duration_ms = started_at
        .map(|t| (chrono::Utc::now() - t).num_milliseconds())
        .unwrap_or(0);
    let mut record = Session::new(session.transcript, session.detected_language, duration_ms);
    // Reuse the id the frontend already saw in `transcript:segment` events
    // for this recording, rather than the fresh one `Session::new` mints.
    record.id = session.id.to_string();

    let repository = SessionRepository::new(pool.clone());
    repository.save(&record).await.map_err(|e| e.to_string())?;

    Ok(record.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

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
        let task = tokio::spawn(async move { session });

        let result = finish_recording(stop_tx, finished_capture_thread(), task, None, &pool).await;

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
    }

    /// Regression test for issue #46: a panicked capture thread must
    /// surface as an `Err`, not panic `stop_recording` itself and leave
    /// the caller unable to reset the state machine back to `Idle`.
    #[tokio::test]
    async fn finish_recording_returns_err_when_capture_thread_panics() {
        let pool = test_pool().await;
        let (stop_tx, _stop_rx) = std::sync::mpsc::channel::<()>();

        let panicked_thread = std::thread::spawn(|| panic!("simulated capture thread panic"));
        let task = tokio::spawn(async { TranscriptionSession::new() });

        let result = finish_recording(stop_tx, panicked_thread, task, None, &pool).await;

        let err = result.expect_err("a panicked capture thread should surface as an error");
        assert!(err.contains("panicked"));
    }

    /// Regression test for issue #46: a panicked transcription task must
    /// surface as an `Err` too, for the same reason.
    #[tokio::test]
    async fn finish_recording_returns_err_when_transcription_task_panics() {
        let pool = test_pool().await;
        let (stop_tx, _stop_rx) = std::sync::mpsc::channel::<()>();

        let task: JoinHandle<TranscriptionSession> =
            tokio::spawn(async { panic!("simulated transcription task panic") });

        let result = finish_recording(stop_tx, finished_capture_thread(), task, None, &pool).await;

        assert!(result.is_err());
    }
}
