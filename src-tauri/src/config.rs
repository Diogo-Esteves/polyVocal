use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::warn;

/// Application settings persisted to `config.toml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    /// Selected input device ID, or `None` to use the system default.
    pub input_device: Option<String>,
    /// Default target language for translation.
    pub target_lang: Option<String>,
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
}
