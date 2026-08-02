use crate::audio::capture::AudioCapture;
use crate::audio::chunker::FrameChunker;
use crate::audio::device;
use crate::audio::state::AudioState;
use crate::models::manager::ModelManager;
use crate::storage::models::Session;
use crate::storage::repository::SessionRepository;
use crate::transcription::engine::TranscriptionEngine;
use crate::transcription::pipeline::RecordingPipeline;
use crate::transcription::session::TranscriptionSession;
use crate::vad::segmenter::SpeechSegmenter;
use crate::vad::silero::{SileroVad, SILERO_FRAME_SIZE};
use sqlx::SqlitePool;
use tauri::{AppHandle, Manager, State};
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

    let whisper_path = ModelManager::new(models_dir.clone())
        .active_model_path()
        .ok_or_else(|| "No active Whisper model — download one in Settings".to_string())?;
    let engine = TranscriptionEngine::load(whisper_path).map_err(|e| e.to_string())?;

    // TODO: move to ModelManager once Silero has a registry entry of its own.
    let silero_path = models_dir.join("silero_vad.onnx");
    let scorer = SileroVad::load(silero_path).map_err(|e| e.to_string())?;
    let segmenter = SpeechSegmenter::new(scorer, VAD_THRESHOLD, VAD_MIN_SILENCE_FRAMES);
    let pipeline = RecordingPipeline::new(segmenter, engine);

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

        while let Some(chunk) = rx.recv().await {
            for frame in chunker.push(&chunk) {
                if let Err(e) = pipeline.push_frame(&frame) {
                    tracing::error!("transcription pipeline error: {e}");
                }
            }
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
    let record = Session::new(session.transcript, session.detected_language, duration_ms);

    let repository = SessionRepository::new(pool.inner().clone());
    repository.save(&record).await.map_err(|e| e.to_string())?;

    slot.audio_state = slot.audio_state.clone().finalize()?;

    Ok(record.id)
}
