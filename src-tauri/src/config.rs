use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::warn;

/// Application settings persisted to `config.toml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Selected input device ID, or `None` to use the system default.
    pub input_device: Option<String>,
    /// Default target language for translation.
    pub target_lang: Option<String>,
    /// Enable streaming partial transcriptions (real-time text while speaking).
    pub streaming_partials_enabled: bool,
}

/// Loads configuration from the given path. Returns `AppConfig::default()` if
/// the file is missing or if there's any error reading or parsing it (logged
/// as a warning, except for missing files which are the expected first-launch
/// case and aren't logged).
pub fn load(path: &Path) -> AppConfig {
    match std::fs::read_to_string(path) {
        Ok(contents) => match toml::from_str(&contents) {
            Ok(config) => config,
            Err(e) => {
                warn!("failed to parse config file: {}", e);
                AppConfig::default()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // First launch — expected, not a problem
            AppConfig::default()
        }
        Err(e) => {
            warn!("failed to read config file: {}", e);
            AppConfig::default()
        }
    }
}

/// Saves configuration to the given path. Creates parent directories as needed.
pub fn save(path: &Path, config: &AppConfig) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = toml::to_string_pretty(config)?;
    std::fs::write(path, contents)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_missing_file_returns_default() {
        let temp_dir = tempfile::tempdir().unwrap();
        let missing_path = temp_dir.path().join("nonexistent.toml");
        let config = load(&missing_path);
        assert_eq!(config, AppConfig::default());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("config.toml");

        let original = AppConfig {
            input_device: Some("device_123".to_string()),
            target_lang: Some("en".to_string()),
            streaming_partials_enabled: true,
        };

        save(&path, &original).unwrap();
        let loaded = load(&path);
        assert_eq!(loaded, original);
    }

    #[test]
    fn test_load_corrupt_file_returns_default() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("bad.toml");
        std::fs::write(&path, "this is not valid toml: {{{").unwrap();
        let config = load(&path);
        assert_eq!(config, AppConfig::default());
    }

    #[test]
    fn test_load_toml_with_missing_new_field_preserves_existing_fields() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("config.toml");
        // Write a TOML file with only the pre-existing fields (no streaming_partials_enabled).
        let contents = r#"input_device = "device_123"
target_lang = "en"
"#;
        std::fs::write(&path, contents).unwrap();
        let config = load(&path);
        // Verify that the existing fields are preserved and the missing field defaults to false.
        assert_eq!(config.input_device, Some("device_123".to_string()));
        assert_eq!(config.target_lang, Some("en".to_string()));
        assert!(!config.streaming_partials_enabled);
    }
}
