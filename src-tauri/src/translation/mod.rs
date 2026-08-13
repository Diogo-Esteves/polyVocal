/// Translation module.
///
/// Responsible for:
/// - Running local OPUS-MT (MarianMT) inference via `candle`, per DEC-010 —
///   no network, no sidecar process
/// - Returning translated text for display
pub mod engine;
pub mod tokenizer;

/// Supported language codes for translation (MVP).
///
/// The backend never reads this itself — routing is driven by the model
/// registry — but it is the declared source of truth the frontend's own
/// language list is kept in step with (see `src/src/main.rs`), so it stays
/// here rather than becoming an undocumented constant on the UI side.
#[allow(dead_code)]
pub const SUPPORTED_LANGUAGES: &[(&str, &str)] =
    &[("en", "English"), ("pt", "Portuguese"), ("es", "Spanish")];
