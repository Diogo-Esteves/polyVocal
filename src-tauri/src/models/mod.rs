/// Model management module.
///
/// Responsible for:
/// - Listing available Whisper model sizes
/// - Tracking download state and local availability
/// - Downloading models from Hugging Face / official sources
/// - Setting the active model for inference
pub mod manager;
pub mod registry;
