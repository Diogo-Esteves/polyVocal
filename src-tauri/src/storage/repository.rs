#![allow(dead_code)]

use super::models::Session;
use anyhow::Result;
use sqlx::SqlitePool;

pub struct SessionRepository {
    pool: SqlitePool,
}

impl SessionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn save(&self, session: &Session) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sessions (id, created_at, duration_ms, language, transcript, translation, target_lang, synced)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&session.id)
        .bind(&session.created_at)
        .bind(session.duration_ms)
        .bind(&session.language)
        .bind(&session.transcript)
        .bind(&session.translation)
        .bind(&session.target_lang)
        .bind(session.synced)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list(&self, limit: i64, offset: i64) -> Result<Vec<Session>> {
        let sessions = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(sessions)
    }

    pub async fn get(&self, id: &str) -> Result<Option<Session>> {
        let session = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(session)
    }

    pub async fn update_translation(
        &self,
        id: &str,
        translation: &str,
        target_lang: &str,
    ) -> Result<()> {
        sqlx::query("UPDATE sessions SET translation = ?, target_lang = ? WHERE id = ?")
            .bind(translation)
            .bind(target_lang)
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::Session;
    use sqlx::sqlite::SqlitePoolOptions;

    // A single connection: sqlx pools each connection to a distinct
    // in-memory database, so >1 connection would see empty tables.
    async fn test_pool() -> SqlitePool {
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

    #[tokio::test]
    async fn test_save_and_get_roundtrip() {
        let repository = SessionRepository::new(test_pool().await);
        let session = Session::new("Hello world".to_string(), Some("en".to_string()), 1000);
        repository
            .save(&session)
            .await
            .expect("save should succeed");

        let fetched = repository
            .get(&session.id)
            .await
            .expect("get should succeed")
            .expect("session should exist");

        assert_eq!(fetched.id, session.id);
        assert_eq!(fetched.transcript, "Hello world");
        assert_eq!(fetched.language.as_deref(), Some("en"));
    }

    #[tokio::test]
    async fn test_get_returns_none_for_unknown_id() {
        let repository = SessionRepository::new(test_pool().await);

        let fetched = repository
            .get("nonexistent")
            .await
            .expect("get should succeed");

        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_list_orders_newest_first_and_respects_pagination() {
        let repository = SessionRepository::new(test_pool().await);
        for i in 0..3 {
            let mut session = Session::new(format!("transcript {i}"), None, 100);
            // created_at is set to "now" by Session::new; space timestamps out so
            // ORDER BY created_at DESC has a deterministic order to assert on.
            session.created_at = format!("2026-01-0{}T00:00:00Z", i + 1);
            repository
                .save(&session)
                .await
                .expect("save should succeed");
        }

        let page = repository.list(2, 0).await.expect("list should succeed");
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].transcript, "transcript 2");
        assert_eq!(page[1].transcript, "transcript 1");

        let next_page = repository.list(2, 2).await.expect("list should succeed");
        assert_eq!(next_page.len(), 1);
        assert_eq!(next_page[0].transcript, "transcript 0");
    }

    #[tokio::test]
    async fn test_delete_removes_session() {
        let repository = SessionRepository::new(test_pool().await);
        let session = Session::new("to be deleted".to_string(), None, 100);
        repository
            .save(&session)
            .await
            .expect("save should succeed");

        repository
            .delete(&session.id)
            .await
            .expect("delete should succeed");

        let fetched = repository
            .get(&session.id)
            .await
            .expect("get should succeed");
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn test_delete_unknown_id_is_a_noop() {
        let repository = SessionRepository::new(test_pool().await);

        repository
            .delete("nonexistent")
            .await
            .expect("delete of unknown id should not error");
    }

    #[tokio::test]
    async fn test_update_translation_persists_translation_and_target_lang() {
        let repository = SessionRepository::new(test_pool().await);
        let session = Session::new("Hello".to_string(), Some("en".to_string()), 100);
        repository
            .save(&session)
            .await
            .expect("save should succeed");

        repository
            .update_translation(&session.id, "Olá", "pt")
            .await
            .expect("update_translation should succeed");

        let fetched = repository
            .get(&session.id)
            .await
            .expect("get should succeed")
            .expect("session should exist");
        assert_eq!(fetched.translation.as_deref(), Some("Olá"));
        assert_eq!(fetched.target_lang.as_deref(), Some("pt"));
    }
}
