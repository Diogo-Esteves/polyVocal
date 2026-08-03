use crate::storage::repository::SessionRepository;
use crate::translation::client::TranslationClient;
use sqlx::SqlitePool;
use tauri::State;

/// Default LibreTranslate endpoint — matches the `libretranslate` service in
/// `docker-compose.yml`. Override with `LIBRETRANSLATE_URL` (e.g. to point at
/// a remote instance) and `LIBRETRANSLATE_API_KEY` if the instance requires one.
const DEFAULT_BASE_URL: &str = "http://localhost:5000";

fn translation_client() -> TranslationClient {
    let base_url =
        std::env::var("LIBRETRANSLATE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
    let api_key = std::env::var("LIBRETRANSLATE_API_KEY").ok();
    TranslationClient::new(base_url, api_key)
}

/// Translate a stored session's transcript into `target_lang` and persist
/// the result back onto the session. Uses the session's own detected source
/// language rather than a hardcoded one; sessions predating language
/// detection (or where it failed) fall back to `"auto"` so LibreTranslate
/// detects it itself.
async fn translate_session(
    repository: &SessionRepository,
    client: &TranslationClient,
    session_id: &str,
    target_lang: &str,
) -> Result<String, String> {
    let session = repository
        .get(session_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("session not found: {session_id}"))?;

    let source_lang = session.language.as_deref().unwrap_or("auto");

    let translated = client
        .translate(&session.transcript, source_lang, target_lang)
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
    pool: State<'_, SqlitePool>,
    session_id: String,
    target_lang: String,
) -> Result<String, String> {
    let repository = SessionRepository::new(pool.inner().clone());
    let client = translation_client();
    translate_session(&repository, &client, &session_id, &target_lang).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::Session;
    use serde_json::json;
    use sqlx::sqlite::SqlitePoolOptions;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

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

    /// Echoes the request's `source` field back in the fake translation, so
    /// tests can assert which source language the command actually sent
    /// without needing a real LibreTranslate instance.
    struct EchoSource;

    impl Respond for EchoSource {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            let source = body["source"].as_str().unwrap_or("");
            ResponseTemplate::new(200)
                .set_body_json(json!({ "translatedText": format!("[{source}] translated") }))
        }
    }

    #[tokio::test]
    async fn test_translate_session_uses_detected_source_language() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/translate"))
            .respond_with(EchoSource)
            .expect(1)
            .mount(&mock_server)
            .await;

        let pool = test_pool().await;
        let repository = SessionRepository::new(pool);
        let session = Session::new("Hello world".to_string(), Some("en".to_string()), 1000);
        repository
            .save(&session)
            .await
            .expect("session should save");

        let client = TranslationClient::new(mock_server.uri(), None);
        let translated = translate_session(&repository, &client, &session.id, "pt")
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
    async fn test_translate_session_falls_back_to_auto_when_source_language_unknown() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/translate"))
            .respond_with(EchoSource)
            .expect(1)
            .mount(&mock_server)
            .await;

        let pool = test_pool().await;
        let repository = SessionRepository::new(pool);
        // No detected language — e.g. an older session, or detection failed.
        let session = Session::new("Hi".to_string(), None, 500);
        repository
            .save(&session)
            .await
            .expect("session should save");

        let client = TranslationClient::new(mock_server.uri(), None);
        let translated = translate_session(&repository, &client, &session.id, "es")
            .await
            .expect("translation should succeed");

        assert_eq!(translated, "[auto] translated");
    }

    #[tokio::test]
    async fn test_translate_session_errors_for_unknown_session() {
        let mock_server = MockServer::start().await;
        let pool = test_pool().await;
        let repository = SessionRepository::new(pool);
        let client = TranslationClient::new(mock_server.uri(), None);

        let result = translate_session(&repository, &client, "nonexistent", "pt").await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn test_translate_session_propagates_client_error_without_persisting() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/translate"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let pool = test_pool().await;
        let repository = SessionRepository::new(pool);
        let session = Session::new("Hello".to_string(), Some("en".to_string()), 100);
        repository
            .save(&session)
            .await
            .expect("session should save");

        let client = TranslationClient::new(mock_server.uri(), None);
        let result = translate_session(&repository, &client, &session.id, "pt").await;

        assert!(result.is_err());

        let unchanged = repository
            .get(&session.id)
            .await
            .expect("get should succeed")
            .expect("session should still exist");
        assert_eq!(unchanged.translation, None);
        assert_eq!(unchanged.target_lang, None);
    }

    /// Exercises the real HTTP round-trip against an actual LibreTranslate
    /// instance — not part of the default suite (needs `docker compose up -d`
    /// from the repo root first, and LibreTranslate can take a while to
    /// fetch its language models on first start), run manually with
    /// `--ignored` when touching the client/command wiring. The mocked
    /// tests above cover the command's logic (source-language fallback,
    /// persistence, error handling) in the default suite.
    #[tokio::test]
    #[ignore]
    async fn test_real_libretranslate_translates_end_to_end() {
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

        let client = translation_client();
        let translated = translate_session(&repository, &client, &session.id, "pt")
            .await
            .expect("real LibreTranslate call should succeed");

        assert!(!translated.is_empty());
        assert_ne!(translated, session.transcript);
    }
}
