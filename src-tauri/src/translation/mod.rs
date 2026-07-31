#![allow(dead_code)]

/// Translation module.
///
/// Responsible for:
/// - Sending transcript text to a local translation engine
///   (LibreTranslate or Argos Translate running as a sidecar)
/// - Returning translated text for display
/// - (Future) Cloud translation providers as an optional upgrade
pub mod client;

/// Supported language codes for translation (MVP).
pub const SUPPORTED_LANGUAGES: &[(&str, &str)] =
    &[("en", "English"), ("pt", "Portuguese"), ("es", "Spanish")];
