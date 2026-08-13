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
}
