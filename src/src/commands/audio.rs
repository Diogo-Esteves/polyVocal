use serde::{Deserialize, Serialize};

/// Mirrors the `transcript:segment` event payload emitted by the Rust
/// backend (DEC-007) — only the fields this screen renders are declared;
/// serde ignores the rest.
#[derive(Deserialize, Clone)]
pub struct TranscriptSegment {
    pub text: String,
    pub language: String,
}

/// Mirrors the `audio:level` event payload emitted by the Rust backend
/// (#76) — a single smoothed RMS amplitude in `[0, 1]`, sampled from the mic
/// roughly 20 times a second while recording.
#[derive(Deserialize, Clone)]
pub struct AudioLevel {
    pub level: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRecordingArgs {
    pub device_id: Option<String>,
}

/// Mirrors `audio::InputDevice` — a command return value, same reasoning as
/// `ModelInfo` above.
#[derive(Deserialize, Clone)]
pub struct InputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Lists available input devices.
pub async fn list_input_devices() -> Result<Vec<InputDevice>, String> {
    tauri_sys::core::invoke_result::<Vec<InputDevice>, String>("list_input_devices", ()).await
}

/// Starts recording from the specified device (or the default if device_id is None).
pub async fn start_recording(device_id: Option<String>) -> Result<(), String> {
    let args = StartRecordingArgs { device_id };
    tauri_sys::core::invoke_result::<(), String>("start_recording", args).await
}

/// Stops the current recording and returns the session ID.
pub async fn stop_recording() -> Result<String, String> {
    tauri_sys::core::invoke_result::<String, String>("stop_recording", ()).await
}
