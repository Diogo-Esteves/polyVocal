use anyhow::Result;
use serde::{Deserialize, Serialize};

/// HTTP client for a locally-running LibreTranslate instance.
pub struct TranslationClient {
    base_url: String,
    api_key: Option<String>,
}

impl TranslationClient {
    /// Create a client pointing at a local LibreTranslate server.
    pub fn new(base_url: impl Into<String>, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.into(),
            api_key,
        }
    }

    /// Translate `text` from `source_lang` to `target_lang`.
    /// Pass `"auto"` as `source_lang` to let the engine detect it.
    pub async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<String> {
        let client = reqwest::Client::new();

        let body = TranslateRequest {
            q: text.to_string(),
            source: source_lang.to_string(),
            target: target_lang.to_string(),
            api_key: self.api_key.clone(),
        };

        let response: TranslateResponse = client
            .post(format!("{}/translate", self.base_url))
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(response.translated_text)
    }
}

#[derive(Serialize)]
struct TranslateRequest {
    q: String,
    source: String,
    target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key: Option<String>,
}

#[derive(Deserialize)]
struct TranslateResponse {
    #[serde(rename = "translatedText")]
    translated_text: String,
}
