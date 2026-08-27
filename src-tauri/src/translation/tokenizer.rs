use anyhow::{anyhow, Result};
use rust_tokenizers::tokenizer::{MarianTokenizer as RustMarianTokenizer, Tokenizer as _};
use std::path::Path;

/// Tokenizer for a single OPUS-MT (MarianMT) checkpoint.
///
/// Thin wrapper around `rust_tokenizers`' `MarianTokenizer`, which loads
/// `source.spm`/`vocab.json` directly (a raw SentencePiece model plus a
/// plain piece→id JSON — *not* a HF `tokenizers` fast-tokenizer JSON) and
/// reimplements SentencePiece's segmentation algorithm and Marian's
/// `>>lang<<` prefix handling in pure Rust — no FFI to the reference
/// SentencePiece C++ library, which would conflict with `ort`'s bundled
/// protobuf (see the `rust_tokenizers` dependency comment in Cargo.toml).
pub struct MarianTokenizer {
    inner: RustMarianTokenizer,
}

impl MarianTokenizer {
    /// Load from a translation model's directory (see
    /// `ModelManager::ensure_translation_model`), which contains
    /// `source.spm` and `vocab.json`.
    pub fn load(model_dir: &Path) -> Result<Self> {
        // Marian doesn't lower-case source text (matches HF's
        // `MarianTokenizer` default `do_lower_case=False`).
        let inner = RustMarianTokenizer::from_files(
            model_dir.join("vocab.json"),
            model_dir.join("source.spm"),
            false,
        )
        .map_err(|e| {
            anyhow!(
                "failed to load Marian tokenizer from {}: {e}",
                model_dir.display()
            )
        })?;

        Ok(Self { inner })
    }

    /// Segment `text` into vocabulary ids.
    pub fn encode(&self, text: &str) -> Result<Vec<u32>> {
        let tokens = self.inner.tokenize(text);
        Ok(self
            .inner
            .convert_tokens_to_ids(&tokens)
            .into_iter()
            .map(|id| id as u32)
            .collect())
    }

    /// Join ids back into text, cleaning up tokenization spaces (e.g. " ,"
    /// -> ",") the way HF's slow tokenizers do by default. The first piece
    /// of a sentence is always word-initial (SentencePiece's `▁` marker),
    /// which decodes to a leading space that HF's own `decode()` doesn't
    /// strip either — trimmed here instead.
    pub fn decode(&self, ids: &[u32]) -> String {
        let ids: Vec<i64> = ids.iter().map(|&id| id as i64).collect();
        self.inner.decode(&ids, true, true).trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_missing_directory_fails() {
        let result = MarianTokenizer::load(Path::new("/nonexistent/translation/model"));
        assert!(result.is_err());
    }

    /// Exercises real MarianTokenizer encode/decode round-trip end to end —
    /// downloads the real `Helsinki-NLP/opus-mt-en-es` checkpoint (~300 MB,
    /// cached across runs under the OS temp dir) and tests tokenization
    /// against real SentencePiece `source.spm`/`vocab.json` files. Not part
    /// of the default suite (network + a large download on first run), run
    /// manually with `--ignored` when touching the tokenizer/model registry
    /// wiring. The test above covers the error path; this one catches
    /// regressions in tokenizer loading and encode/decode behavior.
    #[tokio::test]
    #[ignore]
    async fn test_real_marian_tokenizer_round_trip_en_es() {
        let models_dir = std::env::temp_dir().join("polyvocal_test_real_translation_models");
        let manager = crate::models::manager::ModelManager::new(models_dir);
        let model_dir = manager
            .ensure_translation_model(
                &crate::models::registry::TranslationModel::EnEs,
                &crate::models::downloader::ReqwestDownloader,
            )
            .await
            .expect("should download Helsinki-NLP/opus-mt-en-es checkpoint");

        let tokenizer = MarianTokenizer::load(&model_dir)
            .expect("should load MarianTokenizer from downloaded model");

        let input = "Hello, how are you?";
        let ids = tokenizer
            .encode(input)
            .expect("should encode input string to ids");
        assert!(!ids.is_empty(), "encoded ids should not be empty");

        let decoded = tokenizer.decode(&ids);
        assert!(!decoded.is_empty(), "decoded string should not be empty");

        let decoded_lower = decoded.to_lowercase();
        assert!(
            decoded_lower.contains("hello") || decoded_lower.contains("hi"),
            "decoded output should contain recognizable greeting word, got: {decoded}"
        );
        assert!(
            decoded_lower.contains("how") || decoded_lower.contains("are"),
            "decoded output should contain recognizable question words, got: {decoded}"
        );
        assert!(
            decoded_lower.contains("you"),
            "decoded output should contain 'you', got: {decoded}"
        );
    }
}
