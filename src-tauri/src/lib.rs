mod audio;
mod commands;
mod models;
mod storage;
mod sync;
mod transcription;
mod translation;

use tauri::Manager;
use tracing::info;

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
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running PolyVocal");
}
