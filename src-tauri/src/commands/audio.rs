use crate::audio::device;
use serde::{Deserialize, Serialize};
use tauri::State;

#[tauri::command]
pub async fn list_input_devices() -> Result<Vec<crate::audio::InputDevice>, String> {
    device::list_input_devices().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_recording(device_id: Option<String>) -> Result<(), String> {
    // TODO: start AudioCapture, wire PCM stream into TranscriptionEngine,
    //       emit "transcript:partial" events to frontend
    Ok(())
}

#[tauri::command]
pub async fn stop_recording() -> Result<String, String> {
    // TODO: stop AudioCapture, finalise TranscriptionSession, persist to DB,
    //       return the session ID
    Ok("session-id-placeholder".into())
}
