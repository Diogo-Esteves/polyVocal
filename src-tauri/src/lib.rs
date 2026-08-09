pub mod audio;
mod commands;
pub mod models;
mod storage;
mod sync;
pub mod transcription;
mod translation;
pub mod vad;

use tracing::{info, warn};

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "polyvocal=debug".into()),
        )
        .init();

    info!("Starting PolyVocal");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .manage(commands::audio::RecordingState::default())
        .invoke_handler(tauri::generate_handler![
            // Audio
            commands::audio::list_input_devices,
            commands::audio::start_recording,
            commands::audio::stop_recording,
            // Transcription
            commands::transcription::get_transcript_status,
            // Translation
            commands::translation::translate_text,
            // Storage
            commands::storage::list_sessions,
            commands::storage::get_session,
            commands::storage::delete_session,
            // Models
            commands::models::list_models,
            commands::models::download_model,
            commands::models::set_active_model,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                storage::db::initialise(&app_handle)
                    .await
                    .expect("failed to initialise database");
            });

            // Best-effort: a fresh install can't transcribe until both the
            // VAD model and some Whisper model are present, so provision
            // both automatically. Never fatal — e.g. offline on first
            // launch — the app still opens, just with the existing
            // "no active model" / "VAD model not found" error paths.
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = commands::models::ensure_default_models(&app_handle).await {
                    warn!("failed to provision default models: {e}");
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running PolyVocal");
}
