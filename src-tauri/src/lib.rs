pub mod audio;
mod commands;
pub mod logging;
pub mod models;
mod storage;
pub mod transcription;
mod translation;
pub mod vad;

use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tracing::{error, info, warn};

pub fn run() {
    // Built here rather than inline in `.run()` below so the bundle
    // identifier is available to the logger: the rotating log file has to
    // land in the same app data directory Tauri resolves for the database,
    // and `app.path()` doesn't exist until `.setup()` — by which point the
    // startup lines below would already have been lost.
    let context = tauri::generate_context!();

    // Held until `run()` returns: dropping the guard shuts down the file
    // writer's background thread and discards anything still buffered.
    let _log_guard = logging::init(&context.config().identifier);

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
            // Translation
            commands::translation::translate_text,
            // Storage
            commands::storage::list_sessions,
            commands::storage::get_session,
            commands::storage::delete_session,
            commands::storage::export_session_txt,
            commands::storage::export_session_srt,
            // Models
            commands::models::list_models,
            commands::models::download_model,
            commands::models::set_active_model,
        ])
        .setup(|app| {
            // Blocking (not spawned): every #[tauri::command] that takes
            // State<'_, SqlitePool> needs app.manage(pool) to have already
            // happened — a spawned task raced the frontend's own mount-time
            // commands (see issue #48). DB init failure is unrecoverable
            // (DEC-014 tier 3): show a blocking dialog and exit cleanly
            // instead of the previous `.expect()`, which silently aborted
            // the whole process in release builds.
            let app_handle = app.handle().clone();
            if let Err(e) = tauri::async_runtime::block_on(storage::db::initialise(&app_handle)) {
                error!("failed to initialise database: {e}");
                app_handle
                    .dialog()
                    .message(format!(
                        "PolyVocal couldn't start because the local database \
                         could not be opened:\n\n{e}"
                    ))
                    .title("Database Error")
                    .kind(MessageDialogKind::Error)
                    .blocking_show();
                std::process::exit(1);
            }

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
        .run(context)
        .expect("error while running PolyVocal");
}
