//! Persisted cache of startup calibration verdicts (#185).
//!
//! RTF is a property of (machine, tier, decode strategy) that doesn't
//! change between recordings on the same install, so re-running the full
//! calibration walk (tens of seconds of real inference, #144 Phase 2) on
//! every `start_recording` call is wasted work once a machine's numbers
//! are known. Cache just the verdict (tier + strategy), not the RTF
//! samples themselves — samples exist only for the observability log
//! line, so there's nothing to gain from persisting them.

use crate::models::registry::ModelSize;
use crate::transcription::engine::DecodeStrategy;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CacheKey {
    app_version: String,
    starting_tier: ModelSize,
    logical_cores: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedCalibration {
    pub model_size: ModelSize,
    pub strategy: DecodeStrategy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CacheEntry {
    key: CacheKey,
    result: CachedCalibration,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct CacheFile {
    entries: Vec<CacheEntry>,
}

fn current_key(starting_tier: ModelSize) -> CacheKey {
    CacheKey {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        starting_tier,
        logical_cores: std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4),
    }
}

/// Reads a cached calibration verdict matching the current (app version,
/// starting tier, logical core count) key. Any miss, invalidation, or
/// read/parse error is treated as a cache miss (falls through to real
/// calibration) rather than propagated — a corrupt or stale cache file
/// must never fail `start_recording`.
pub fn read(path: &Path, starting_tier: ModelSize) -> Option<CachedCalibration> {
    let contents = std::fs::read_to_string(path).ok()?;
    let file: CacheFile = serde_json::from_str(&contents).ok()?;
    let key = current_key(starting_tier);
    file.entries
        .into_iter()
        .find(|entry| entry.key == key)
        .map(|entry| entry.result)
}

/// Writes (or replaces) the cache entry for `starting_tier`'s current key.
/// Errors are logged, not propagated — a failed cache write must never
/// fail the recording that triggered it.
pub fn write(
    path: &Path,
    starting_tier: ModelSize,
    model_size: ModelSize,
    strategy: DecodeStrategy,
) {
    let key = current_key(starting_tier);
    let mut file: CacheFile = std::fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default();
    file.entries.retain(|entry| entry.key != key);
    file.entries.push(CacheEntry {
        key,
        result: CachedCalibration {
            model_size,
            strategy,
        },
    });
    match serde_json::to_string_pretty(&file) {
        Ok(json) => {
            if let Err(e) = std::fs::write(path, json) {
                tracing::warn!("failed to write calibration cache: {e}");
            }
        }
        Err(e) => tracing::warn!("failed to serialize calibration cache: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_write_then_read_round_trips_for_same_starting_tier() {
        let dir = tempfile::tempdir().expect("tempdir should create");
        let path = dir.path().join("cache.json");

        let model_size = ModelSize::Small;
        let strategy = DecodeStrategy::BeamSearch { beam_size: 5 };
        write(&path, ModelSize::Medium, model_size, strategy);

        let result = read(&path, ModelSize::Medium).expect("read should succeed");
        assert_eq!(result.model_size, model_size);
        assert_eq!(result.strategy, strategy);
    }

    #[test]
    fn test_read_misses_for_different_starting_tier() {
        let dir = tempfile::tempdir().expect("tempdir should create");
        let path = dir.path().join("cache.json");

        write(
            &path,
            ModelSize::Medium,
            ModelSize::Small,
            DecodeStrategy::Greedy,
        );

        let result = read(&path, ModelSize::Small);
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_returns_none_for_missing_file() {
        let dir = tempfile::tempdir().expect("tempdir should create");
        let path = dir.path().join("nonexistent.json");

        let result = read(&path, ModelSize::Medium);
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_returns_none_for_corrupt_json() {
        let dir = tempfile::tempdir().expect("tempdir should create");
        let path = dir.path().join("corrupt.json");

        let mut file = std::fs::File::create(&path).expect("file should create");
        file.write_all(b"not valid json")
            .expect("write should succeed");
        drop(file);

        let result = read(&path, ModelSize::Medium);
        assert_eq!(result, None);
    }

    #[test]
    fn test_write_replaces_existing_entry_for_same_key() {
        let dir = tempfile::tempdir().expect("tempdir should create");
        let path = dir.path().join("cache.json");

        let starting_tier = ModelSize::Medium;
        write(
            &path,
            starting_tier,
            ModelSize::Small,
            DecodeStrategy::Greedy,
        );
        write(
            &path,
            starting_tier,
            ModelSize::Base,
            DecodeStrategy::BeamSearch { beam_size: 5 },
        );

        let result = read(&path, starting_tier).expect("read should succeed");
        assert_eq!(result.model_size, ModelSize::Base);
        assert_eq!(result.strategy, DecodeStrategy::BeamSearch { beam_size: 5 });
    }
}
