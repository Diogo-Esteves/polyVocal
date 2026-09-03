use serde::{Deserialize, Serialize};

/// Mirrors `models::registry::ModelSize` — `rename_all = "lowercase"` on the
/// backend, so this must serialize/deserialize the same way to match.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModelSize {
    Tiny,
    Base,
    Small,
    Medium,
}

impl ModelSize {
    /// "Fast/Balanced/Best" rather than "tiny/base/small/medium" —
    /// `../../design/DESIGN.md` principle 5 ("Languages, not files") reads
    /// the same way for Whisper sizes: "tiny/base/small/medium" is
    /// vocabulary for people who already know what a Whisper model is.
    /// `Base` is a real, selectable backend size but sits between Fast and
    /// Balanced with no room in a three-way picker, so it's the one size
    /// Settings doesn't surface — a judgment call, not a backend removal
    /// (see `MODEL_PICKER_SIZES` below).
    pub fn label(self) -> &'static str {
        match self {
            ModelSize::Tiny => "Fast",
            ModelSize::Base => "Base",
            ModelSize::Small => "Balanced",
            ModelSize::Medium => "Best",
        }
    }

    /// Matches `models::registry::ModelSize::size_mb` in the backend.
    pub fn size_mb(self) -> u32 {
        match self {
            ModelSize::Tiny => 75,
            ModelSize::Base => 145,
            ModelSize::Small => 465,
            ModelSize::Medium => 1500,
        }
    }
}

/// The three sizes Settings' "Accuracy" picker offers — see
/// `ModelSize::label`'s doc comment on why `Base` is excluded.
pub const MODEL_PICKER_SIZES: [ModelSize; 3] =
    [ModelSize::Tiny, ModelSize::Small, ModelSize::Medium];

/// Mirrors `models::registry::ModelInfo` — a command *return value*,
/// serialized as-is by the backend's own (unrenamed) `Serialize` impl, so
/// no `#[serde(rename_all)]` here — unlike the `*Args` structs below, which
/// are command *arguments* and get auto-camelCased by Tauri.
#[derive(Deserialize, Clone)]
pub struct ModelInfo {
    pub size: ModelSize,
    pub downloaded: bool,
    pub is_active: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadModelArgs {
    size: ModelSize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SetActiveModelArgs {
    size: ModelSize,
}

/// Lists all available Whisper models and their download/active status.
pub async fn list_models() -> Result<Vec<ModelInfo>, String> {
    tauri_sys::core::invoke_result::<Vec<ModelInfo>, String>("list_models", ()).await
}

/// Downloads a Whisper model of the specified size.
pub async fn download_model(size: ModelSize) -> Result<(), String> {
    let args = DownloadModelArgs { size };
    tauri_sys::core::invoke_result::<(), String>("download_model", args).await
}

/// Sets a Whisper model as active (the one used for transcription).
pub async fn set_active_model(size: ModelSize) -> Result<(), String> {
    let args = SetActiveModelArgs { size };
    tauri_sys::core::invoke_result::<(), String>("set_active_model", args).await
}
