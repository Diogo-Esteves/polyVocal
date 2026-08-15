use super::tokenizer::MarianTokenizer;
use crate::models::downloader::ModelDownloader;
use crate::models::manager::ModelManager;
use crate::models::registry::TranslationModel;
use anyhow::{anyhow, Result};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::marian::{Config, MTModel};
use std::path::{Path, PathBuf};
use whatlang::{Detector, Lang};

/// Generation is capped rather than unbounded: a runaway decode (e.g. the
/// model never emitting eos) should degrade to a truncated translation, not
/// hang the calling command indefinitely.
const MAX_GENERATED_TOKENS: usize = 512;

/// Local, offline translation engine — OPUS-MT (MarianMT) models run via
/// `candle`, per DEC-010. Replaces the LibreTranslate HTTP client.
///
/// Deliberately stateless between calls: each `translate` loads only the
/// checkpoint(s) it needs for that pair from disk. Translation is a
/// user-triggered, one-shot action (not per-utterance like transcription),
/// so the reload cost (roughly a second per ~300 MB checkpoint) is an
/// acceptable trade for keeping this a simple, drop-in replacement rather
/// than introducing app-wide model-caching state.
pub struct TranslationEngine {
    models_dir: PathBuf,
}

impl TranslationEngine {
    pub fn new(models_dir: PathBuf) -> Self {
        Self { models_dir }
    }

    /// Translate `text` from `source_lang` to `target_lang` (both ISO
    /// 639-1, e.g. `"en"`), downloading whichever underlying OPUS-MT
    /// checkpoint(s) are needed first. Downloads are skipped for files
    /// already on disk (see `ModelManager::ensure_translation_model`).
    pub async fn translate<D: ModelDownloader>(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
        downloader: &D,
    ) -> Result<String> {
        if text.trim().is_empty() {
            return Ok(String::new());
        }
        if source_lang == target_lang {
            return Ok(text.to_string());
        }

        let pipeline = resolve_pipeline(source_lang, target_lang).ok_or_else(|| {
            anyhow!("unsupported language pair for translation: {source_lang} -> {target_lang}")
        })?;

        let manager = ModelManager::new(self.models_dir.clone());
        let mut current = text.to_string();
        for hop in pipeline {
            let model_dir = manager.ensure_translation_model(&hop, downloader).await?;
            current = tokio::task::spawn_blocking(move || {
                let leg = MarianLeg::load(&model_dir, hop)?;
                leg.translate(&current)
            })
            .await
            .map_err(|e| anyhow!("translation task panicked: {e}"))??;
        }
        Ok(current)
    }
}

/// Resolve `source_lang -> target_lang` into an ordered chain of underlying
/// OPUS-MT checkpoints. Most MVP pairs are a single hop; `pt<->es` has no
/// direct Helsinki-NLP model, so it pivots through English (see DEC-010).
fn resolve_pipeline(source_lang: &str, target_lang: &str) -> Option<Vec<TranslationModel>> {
    use TranslationModel::*;
    match (source_lang, target_lang) {
        ("en", "es") => Some(vec![EnEs]),
        ("es", "en") => Some(vec![EsEn]),
        ("en", "pt") => Some(vec![EnPt]),
        ("pt", "en") => Some(vec![RomanceEn]),
        ("pt", "es") => Some(vec![RomanceEn, EnEs]),
        ("es", "pt") => Some(vec![EsEn, EnPt]),
        _ => None,
    }
}

/// Best-effort source-language detection for sessions with no recorded
/// language (predating language detection, or where it failed), restricted
/// to the languages `resolve_pipeline` can actually route — mirroring the
/// role the LibreTranslate HTTP client's `"auto"` source used to play, but
/// resolved locally up front instead of delegated to the translation
/// server itself.
pub fn detect_language(text: &str) -> Option<&'static str> {
    let detector = Detector::with_allowlist(vec![Lang::Eng, Lang::Por, Lang::Spa]);
    match detector.detect_lang(text)? {
        Lang::Eng => Some("en"),
        Lang::Por => Some("pt"),
        Lang::Spa => Some("es"),
        _ => None,
    }
}

/// A single loaded OPUS-MT checkpoint, translating in one fixed direction.
struct MarianLeg {
    tokenizer: MarianTokenizer,
    model: MTModel,
    config: Config,
    target_prefix: Option<&'static str>,
    device: Device,
}

impl MarianLeg {
    fn load(model_dir: &Path, translation_model: TranslationModel) -> Result<Self> {
        let config: Config =
            serde_json::from_str(&std::fs::read_to_string(model_dir.join("config.json"))?)?;
        let tokenizer = MarianTokenizer::load(model_dir)?;

        let device = Device::Cpu;
        // Safety: `from_mmaped_safetensors` requires the file not be
        // mutated while mapped, which holds here — `model.safetensors` is
        // a read-only download this process (or a concurrent one racing
        // `ensure_translation_model`) only ever writes once, atomically,
        // via a `.part` rename (see `ReqwestDownloader`).
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(
                &[model_dir.join("model.safetensors")],
                DType::F32,
                &device,
            )?
        };
        let model = MTModel::new(&config, vb)?;

        Ok(Self {
            tokenizer,
            model,
            config,
            target_prefix: translation_model.target_prefix(),
            device,
        })
    }

    /// Greedy-decode a translation of `text` through this checkpoint.
    fn translate(mut self, text: &str) -> Result<String> {
        let input = match self.target_prefix {
            Some(prefix) => format!("{prefix}{text}"),
            None => text.to_string(),
        };

        let mut source_ids = self.tokenizer.encode(&input)?;
        source_ids.push(self.config.eos_token_id);
        let source_tensor = Tensor::new(source_ids.as_slice(), &self.device)?.unsqueeze(0)?;
        let encoder_xs = self.model.encoder().forward(&source_tensor, 0)?;

        // Greedy: no temperature/top-p, deterministic output for the same
        // input — appropriate for translation (unlike open-ended
        // generation, there's no reason to want sampling diversity here).
        let mut logits_processor = LogitsProcessor::new(1337, None, None);
        let mut output_ids = vec![self.config.decoder_start_token_id];
        for index in 0..MAX_GENERATED_TOKENS {
            let context_size = if index >= 1 { 1 } else { output_ids.len() };
            let start_pos = output_ids.len().saturating_sub(context_size);
            let input_ids = Tensor::new(&output_ids[start_pos..], &self.device)?.unsqueeze(0)?;
            let logits = self.model.decode(&input_ids, &encoder_xs, start_pos)?;
            let logits = logits.squeeze(0)?;
            let logits = logits.get(logits.dim(0)? - 1)?;
            let token = logits_processor.sample(&logits)?;
            output_ids.push(token);
            if token == self.config.eos_token_id || token == self.config.forced_eos_token_id {
                break;
            }
        }

        // Drop the leading decoder-start token and everything from the
        // first eos onward (decode ran to MAX_GENERATED_TOKENS without one
        // in the truncation case, so this is a no-op there).
        let body: Vec<u32> = output_ids
            .into_iter()
            .skip(1)
            .take_while(|&id| {
                id != self.config.eos_token_id && id != self.config.forced_eos_token_id
            })
            .collect();

        Ok(self.tokenizer.decode(&body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_pipeline_direct_pairs() {
        assert_eq!(
            resolve_pipeline("en", "es"),
            Some(vec![TranslationModel::EnEs])
        );
        assert_eq!(
            resolve_pipeline("es", "en"),
            Some(vec![TranslationModel::EsEn])
        );
        assert_eq!(
            resolve_pipeline("en", "pt"),
            Some(vec![TranslationModel::EnPt])
        );
        assert_eq!(
            resolve_pipeline("pt", "en"),
            Some(vec![TranslationModel::RomanceEn])
        );
    }

    #[test]
    fn test_resolve_pipeline_pivots_pt_es_through_english() {
        assert_eq!(
            resolve_pipeline("pt", "es"),
            Some(vec![TranslationModel::RomanceEn, TranslationModel::EnEs])
        );
        assert_eq!(
            resolve_pipeline("es", "pt"),
            Some(vec![TranslationModel::EsEn, TranslationModel::EnPt])
        );
    }

    #[test]
    fn test_resolve_pipeline_rejects_unsupported_pair() {
        assert_eq!(resolve_pipeline("en", "fr"), None);
        assert_eq!(resolve_pipeline("de", "en"), None);
    }

    #[test]
    fn test_detect_language_identifies_supported_languages() {
        assert_eq!(
            detect_language("The quick brown fox jumps over the lazy dog"),
            Some("en")
        );
        assert_eq!(
            detect_language("El rápido zorro marrón salta sobre el perro perezoso"),
            Some("es")
        );
        assert_eq!(
            detect_language("A rápida raposa marrom pula sobre o cão preguiçoso"),
            Some("pt")
        );
    }

    #[tokio::test]
    async fn test_translate_returns_input_unchanged_for_same_language() {
        struct UnusedDownloader;
        impl ModelDownloader for UnusedDownloader {
            async fn download_to(
                &self,
                _url: &str,
                _dest: &std::path::Path,
                _expected_sha256: &str,
            ) -> Result<()> {
                panic!("should not download anything for a same-language translation");
            }
        }

        let engine = TranslationEngine::new(std::env::temp_dir());
        let result = engine
            .translate("Hello there", "en", "en", &UnusedDownloader)
            .await
            .unwrap();

        assert_eq!(result, "Hello there");
    }

    #[tokio::test]
    async fn test_translate_returns_empty_string_for_empty_input() {
        struct UnusedDownloader;
        impl ModelDownloader for UnusedDownloader {
            async fn download_to(
                &self,
                _url: &str,
                _dest: &std::path::Path,
                _expected_sha256: &str,
            ) -> Result<()> {
                panic!("should not download anything for empty input");
            }
        }

        let engine = TranslationEngine::new(std::env::temp_dir());
        let result = engine
            .translate("   ", "en", "es", &UnusedDownloader)
            .await
            .unwrap();

        assert_eq!(result, "");
    }

    #[tokio::test]
    async fn test_translate_rejects_unsupported_pair_without_downloading() {
        struct UnusedDownloader;
        impl ModelDownloader for UnusedDownloader {
            async fn download_to(
                &self,
                _url: &str,
                _dest: &std::path::Path,
                _expected_sha256: &str,
            ) -> Result<()> {
                panic!("should not download anything for an unsupported pair");
            }
        }

        let engine = TranslationEngine::new(std::env::temp_dir());
        let result = engine
            .translate("Bonjour", "fr", "en", &UnusedDownloader)
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("unsupported"));
    }
}
