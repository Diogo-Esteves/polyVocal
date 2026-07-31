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
