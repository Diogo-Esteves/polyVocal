/// Tauri command handlers — the IPC bridge between the frontend and Rust core.
///
/// Each submodule maps to a domain and exposes `#[tauri::command]` functions
/// that the frontend calls via `invoke()`.
pub mod audio;
pub mod models;
pub mod storage;
pub mod transcription;
pub mod translation;
