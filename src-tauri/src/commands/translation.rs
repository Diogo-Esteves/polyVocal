use crate::models::downloader::ReqwestDownloader;
use crate::storage::repository::SessionRepository;
use crate::translation::engine::{detect_language, TranslationEngine};
use sqlx::SqlitePool;
use std::future::Future;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};

/// Abstraction over "translate text from one language to another", so
/// `translate_session`'s own logic (source-language fallback, persistence,
/// error handling) can be unit tested without running real candle
/// inference against real downloaded OPUS-MT weights. `EngineTranslator`
/// (wrapping `TranslationEngine`) is the production implementation.
trait Translator {
    fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> impl Future<Output = anyhow::Result<String>> + Send;
}

struct EngineTranslator {
    engine: TranslationEngine,
}

impl Translator for EngineTranslator {
    async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> anyhow::Result<String> {
        self.engine
            .translate(text, source_lang, target_lang, &ReqwestDownloader)
            .await
    }
}

fn models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("models"))
}

/// Translate a stored session's transcript into `target_lang` and persist
/// the result back onto the session. Uses the session's own detected
/// source language; sessions predating language detection (or where it
/// failed) fall back to local language detection over the transcript text
/// itself — unlike LibreTranslate, the local OPUS-MT engine has no
/// server-side auto-detect to delegate an `"auto"` source to, so this is
/// resolved to a concrete language up front instead.
async fn translate_session(
    repository: &SessionRepository,
    translator: &impl Translator,
    session_id: &str,
    target_lang: &str,
) -> Result<String, String> {
    let session = repository
        .get(session_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("session not found: {session_id}"))?;

    let source_lang = match session.language.as_deref() {
        Some(lang) => lang.to_string(),
        None => detect_language(&session.transcript)
            .ok_or_else(|| "could not detect the session's source language".to_string())?
            .to_string(),
    };

    let translated = translator
        .translate(&session.transcript, &source_lang, target_lang)
        .await
        .map_err(|e| e.to_string())?;

    repository
        .update_translation(session_id, &translated, target_lang)
        .await
        .map_err(|e| e.to_string())?;

    Ok(translated)
}

#[tauri::command]
pub async fn translate_text(
    app: AppHandle,
    pool: State<'_, SqlitePool>,
    session_id: String,
    target_lang: String,
) -> Result<String, String> {
    let repository = SessionRepository::new(pool.inner().clone());
    let translator = EngineTranslator {
        engine: TranslationEngine::new(models_dir(&app)?),
    };
    translate_session(&repository, &translator, &session_id, &target_lang).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::Session;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        // A single connection: sqlx pools each connection to a distinct
        // in-memory database, so >1 connection would see empty tables.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");
        crate::storage::db::run_migrations(&pool)
            .await
            .expect("migrations should run");
        pool
    }

    /// Echoes the source language it was called with, so tests can assert
    /// which source language `translate_session` actually resolved without
    /// needing real OPUS-MT weights.
    struct EchoSourceTranslator;

    impl Translator for EchoSourceTranslator {
        async fn translate(
            &self,
            _text: &str,
            source_lang: &str,
            _target_lang: &str,
        ) -> anyhow::Result<String> {
            Ok(format!("[{source_lang}] translated"))
        }
    }

    struct FailingTranslator;

    impl Translator for FailingTranslator {
        async fn translate(
            &self,
            _text: &str,
            _source_lang: &str,
            _target_lang: &str,
        ) -> anyhow::Result<String> {
            Err(anyhow::anyhow!("translation backend unavailable"))
        }
    }

    #[tokio::test]
    async fn test_translate_session_uses_detected_source_language() {
        let pool = test_pool().await;
        let repository = SessionRepository::new(pool);
        let session = Session::new("Hello world".to_string(), Some("en".to_string()), 1000);
        repository
            .save(&session)
            .await
            .expect("session should save");

        let translated = translate_session(&repository, &EchoSourceTranslator, &session.id, "pt")
            .await
            .expect("translation should succeed");

        assert_eq!(translated, "[en] translated");

        let updated = repository
            .get(&session.id)
            .await
            .expect("get should succeed")
            .expect("session should still exist");
        assert_eq!(updated.translation.as_deref(), Some("[en] translated"));
        assert_eq!(updated.target_lang.as_deref(), Some("pt"));
    }

    #[tokio::test]
    async fn test_translate_session_falls_back_to_local_detection_when_source_language_unknown() {
        let pool = test_pool().await;
        let repository = SessionRepository::new(pool);
        // No detected language — e.g. an older session, or detection
        // failed at transcription time. The transcript itself is
        // unambiguously English, so local detection should recover it.
        let session = Session::new(
            "The quick brown fox jumps over the lazy dog".to_string(),
            None,
            500,
        );
        repository
            .save(&session)
            .await
            .expect("session should save");

        let translated = translate_session(&repository, &EchoSourceTranslator, &session.id, "es")
            .await
            .expect("translation should succeed");

        assert_eq!(translated, "[en] translated");
    }

    #[tokio::test]
    async fn test_translate_session_errors_when_source_language_cannot_be_detected() {
        let pool = test_pool().await;
        let repository = SessionRepository::new(pool);
        // Digits carry no linguistic signal whatlang can classify.
        let session = Session::new("0123456789".to_string(), None, 500);
        repository
            .save(&session)
            .await
            .expect("session should save");

        let result = translate_session(&repository, &EchoSourceTranslator, &session.id, "es").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("could not detect"));
    }

    #[tokio::test]
    async fn test_translate_session_errors_for_unknown_session() {
        let pool = test_pool().await;
        let repository = SessionRepository::new(pool);

        let result =
            translate_session(&repository, &EchoSourceTranslator, "nonexistent", "pt").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn test_translate_session_propagates_translator_error_without_persisting() {
        let pool = test_pool().await;
        let repository = SessionRepository::new(pool);
        let session = Session::new("Hello".to_string(), Some("en".to_string()), 100);
        repository
            .save(&session)
            .await
            .expect("session should save");

        let result = translate_session(&repository, &FailingTranslator, &session.id, "pt").await;

        assert!(result.is_err());

        let unchanged = repository
            .get(&session.id)
            .await
            .expect("get should succeed")
            .expect("session should still exist");
        assert_eq!(unchanged.translation, None);
        assert_eq!(unchanged.target_lang, None);
    }

    /// Exercises real candle + OPUS-MT inference end to end — downloads the
    /// real `Helsinki-NLP/opus-mt-en-es` checkpoint (~300 MB, cached across
    /// runs under the OS temp dir) and runs real greedy decoding. Not part
    /// of the default suite (network + a large download on first run), run
    /// manually with `--ignored` when touching the engine/tokenizer/model
    /// registry wiring. The tests above cover `translate_session`'s own
    /// logic (source-language fallback, persistence, error handling) in
    /// the default suite via `EchoSourceTranslator`/`FailingTranslator`.
    #[tokio::test]
    #[ignore]
    async fn test_real_candle_translation_en_to_es_end_to_end() {
        let pool = test_pool().await;
        let repository = SessionRepository::new(pool);
        let session = Session::new(
            "Hello, how are you?".to_string(),
            Some("en".to_string()),
            1000,
        );
        repository
            .save(&session)
            .await
            .expect("session should save");

        let models_dir = std::env::temp_dir().join("polyvocal_test_real_translation_models");
        let translator = EngineTranslator {
            engine: TranslationEngine::new(models_dir),
        };

        let translated = translate_session(&repository, &translator, &session.id, "es")
            .await
            .expect("real candle translation should succeed");

        assert!(!translated.is_empty());
        assert_ne!(translated, session.transcript);
    }

    /// Exercises the pt<->es pivot path (no direct Helsinki-NLP model
    /// exists for that pair — see DEC-010): downloads both
    /// `opus-mt-ROMANCE-en` and `opus-mt-en-es` (~600 MB total) and runs
    /// two real translation hops back to back. Not part of the default
    /// suite, same reasoning as above.
    #[tokio::test]
    #[ignore]
    async fn test_real_candle_translation_pt_to_es_pivots_through_english() {
        let pool = test_pool().await;
        let repository = SessionRepository::new(pool);
        let session = Session::new(
            "Bom dia, como você está?".to_string(),
            Some("pt".to_string()),
            1000,
        );
        repository
            .save(&session)
            .await
            .expect("session should save");

        let models_dir = std::env::temp_dir().join("polyvocal_test_real_translation_models");
        let translator = EngineTranslator {
            engine: TranslationEngine::new(models_dir),
        };

        let translated = translate_session(&repository, &translator, &session.id, "es")
            .await
            .expect("real pivoted candle translation should succeed");

        assert!(!translated.is_empty());
        assert_ne!(translated, session.transcript);
    }
}
