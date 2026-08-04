#![allow(dead_code)]

/// Translation module.
///
/// Responsible for:
/// - Running local OPUS-MT (MarianMT) inference via `candle`, per DEC-010 —
///   no network, no sidecar process
/// - Returning translated text for display
pub mod engine;
pub mod tokenizer;

/// Supported language codes for translation (MVP).
pub const SUPPORTED_LANGUAGES: &[(&str, &str)] =
    &[("en", "English"), ("pt", "Portuguese"), ("es", "Spanish")];
