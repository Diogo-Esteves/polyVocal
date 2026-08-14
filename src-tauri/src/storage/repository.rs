use super::models::{
    Session, TranscriptSegment, SESSION_STATUS_COMPLETE, SESSION_STATUS_IN_PROGRESS,
};
use anyhow::Result;
use chrono::Utc;
use sqlx::SqlitePool;

pub struct SessionRepository {
    pool: SqlitePool,
}

impl SessionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Inserts an already-complete session row. Superseded in production by
    /// the incremental `create_in_progress` / `append_segment` / `finalise`
    /// path (DEC-009); kept as the seeding helper the unit tests use to put
    /// a finished session in front of the code under test.
    #[allow(dead_code)]
    pub async fn save(&self, session: &Session) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sessions (id, created_at, duration_ms, language, transcript, translation, target_lang, synced, status)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
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
        .bind(&session.status)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Creates the header row for a recording that has just started, so the
    /// segments streaming in from `start_recording` have a session to attach
    /// to (DEC-009). Duration, final language and `status = complete` are
    /// filled in later by `finalise`.
    pub async fn create_in_progress(&self, id: &str) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sessions (id, created_at, duration_ms, language, transcript, synced, status)
            VALUES (?, ?, 0, NULL, '', 0, ?)
            "#,
        )
        .bind(id)
        .bind(Utc::now().to_rfc3339())
        .bind(SESSION_STATUS_IN_PROGRESS)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Persists one transcribed segment and folds its text into the
    /// session's denormalised `transcript` cache — both in one transaction,
    /// so a crash can never leave the cache disagreeing with the segments it
    /// summarises.
    pub async fn append_segment(&self, segment: &TranscriptSegment) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            r#"
            INSERT INTO segments (id, session_id, start_ms, end_ms, text, language)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&segment.id)
        .bind(&segment.session_id)
        .bind(segment.start_ms)
        .bind(segment.end_ms)
        .bind(&segment.text)
        .bind(&segment.language)
        .execute(&mut *tx)
        .await?;

        // Space-separated, matching `TranscriptionSession::append`'s
        // in-memory concatenation, so the cached text is identical whether it
        // was built incrementally here or written by `finalise` at stop.
        sqlx::query(
            r#"
            UPDATE sessions
               SET transcript = CASE WHEN transcript = '' THEN ? ELSE transcript || ' ' || ? END,
                   language   = COALESCE(?, language)
             WHERE id = ?
            "#,
        )
        .bind(&segment.text)
        .bind(&segment.text)
        .bind(&segment.language)
        .bind(&segment.session_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    /// A session's segments in recording order.
    ///
    /// The read side of the incremental segment storage added in DEC-009.
    /// The frontend renders segments live from `transcript:segment` events
    /// (DEC-007) and history shows the denormalised `transcript`; these
    /// timestamped rows exist so timed exports can be built on them, and
    /// `export_session_srt` is the first consumer.
    pub async fn segments(&self, session_id: &str) -> Result<Vec<TranscriptSegment>> {
        let segments = sqlx::query_as::<_, TranscriptSegment>(
            "SELECT * FROM segments WHERE session_id = ? ORDER BY start_ms, rowid",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(segments)
    }

    /// Marks a recording finished: records its total duration and final
    /// detected language, and flips it to `complete`.
    ///
    /// Written as an upsert rather than a plain `UPDATE` so it stays correct
    /// even if the in-progress row is missing (a `create_in_progress` that
    /// failed, or a session assembled outside `start_recording`) — a
    /// finalised recording should end up persisted either way. `transcript`
    /// is the authoritative in-memory text, which also repairs the cache if
    /// any individual segment write failed mid-recording.
    pub async fn finalise(
        &self,
        id: &str,
        transcript: &str,
        language: Option<&str>,
        duration_ms: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sessions (id, created_at, duration_ms, language, transcript, synced, status)
            VALUES (?, ?, ?, ?, ?, 0, ?)
            ON CONFLICT(id) DO UPDATE SET
                duration_ms = excluded.duration_ms,
                language    = COALESCE(excluded.language, sessions.language),
                transcript  = excluded.transcript,
                status      = excluded.status
            "#,
        )
        .bind(id)
        .bind(Utc::now().to_rfc3339())
        .bind(duration_ms)
        .bind(language)
        .bind(transcript)
        .bind(SESSION_STATUS_COMPLETE)
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

    /// Deletes a session and its segments. There's no real foreign key on
    /// `segments.session_id` (SQLite enforces FKs only with
    /// `PRAGMA foreign_keys = ON`, per connection), so the cascade is done
    /// here — in one transaction, so a session can never survive with its
    /// segments deleted, or vice versa.
    pub async fn delete(&self, id: &str) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM segments WHERE session_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::Session;
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::Row;

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

    #[tokio::test]
    async fn test_create_in_progress_starts_an_empty_session() {
        let repository = SessionRepository::new(test_pool().await);

        repository
            .create_in_progress("session-1")
            .await
            .expect("create_in_progress should succeed");

        let session = repository
            .get("session-1")
            .await
            .expect("get should succeed")
            .expect("session should exist");
        assert_eq!(session.status, SESSION_STATUS_IN_PROGRESS);
        assert_eq!(session.transcript, "");
        assert_eq!(session.duration_ms, 0);
        assert_eq!(session.language, None);
    }

    #[tokio::test]
    async fn test_append_segment_persists_segments_in_order() {
        let repository = SessionRepository::new(test_pool().await);
        repository
            .create_in_progress("session-1")
            .await
            .expect("create_in_progress should succeed");

        for (text, start_ms, end_ms) in [("hello", 0, 900), ("world", 1_200, 2_000)] {
            repository
                .append_segment(&TranscriptSegment::new(
                    "session-1",
                    text,
                    Some("en"),
                    start_ms,
                    end_ms,
                ))
                .await
                .expect("append_segment should succeed");
        }

        let segments = repository
            .segments("session-1")
            .await
            .expect("segments should be queryable");
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "hello");
        assert_eq!(segments[0].start_ms, 0);
        assert_eq!(segments[0].end_ms, 900);
        assert_eq!(segments[0].language.as_deref(), Some("en"));
        assert_eq!(segments[1].text, "world");
        assert_eq!(segments[1].start_ms, 1_200);
        assert_ne!(segments[0].id, segments[1].id);
    }

    #[tokio::test]
    async fn test_append_segment_updates_the_session_transcript_cache() {
        let repository = SessionRepository::new(test_pool().await);
        repository
            .create_in_progress("session-1")
            .await
            .expect("create_in_progress should succeed");

        for text in ["hello", "world"] {
            repository
                .append_segment(&TranscriptSegment::new(
                    "session-1",
                    text,
                    Some("en"),
                    0,
                    900,
                ))
                .await
                .expect("append_segment should succeed");
        }

        let session = repository
            .get("session-1")
            .await
            .expect("get should succeed")
            .expect("session should exist");
        assert_eq!(session.transcript, "hello world");
        assert_eq!(session.language.as_deref(), Some("en"));
    }

    #[tokio::test]
    async fn test_segments_are_scoped_to_their_session() {
        let repository = SessionRepository::new(test_pool().await);
        for id in ["session-1", "session-2"] {
            repository
                .create_in_progress(id)
                .await
                .expect("create_in_progress should succeed");
            repository
                .append_segment(&TranscriptSegment::new(id, id, None, 0, 100))
                .await
                .expect("append_segment should succeed");
        }

        let segments = repository
            .segments("session-1")
            .await
            .expect("segments should be queryable");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].text, "session-1");
    }

    #[tokio::test]
    async fn test_finalise_completes_an_in_progress_session() {
        let repository = SessionRepository::new(test_pool().await);
        repository
            .create_in_progress("session-1")
            .await
            .expect("create_in_progress should succeed");
        repository
            .append_segment(&TranscriptSegment::new(
                "session-1",
                "hello",
                Some("en"),
                0,
                900,
            ))
            .await
            .expect("append_segment should succeed");

        repository
            .finalise("session-1", "hello world", Some("en"), 4_200)
            .await
            .expect("finalise should succeed");

        let session = repository
            .get("session-1")
            .await
            .expect("get should succeed")
            .expect("session should exist");
        assert_eq!(session.status, SESSION_STATUS_COMPLETE);
        assert_eq!(session.duration_ms, 4_200);
        assert_eq!(session.language.as_deref(), Some("en"));
        // The in-memory transcript wins over the incrementally built cache.
        assert_eq!(session.transcript, "hello world");
    }

    #[tokio::test]
    async fn test_finalise_keeps_the_incrementally_detected_language_when_none_is_given() {
        let repository = SessionRepository::new(test_pool().await);
        repository
            .create_in_progress("session-1")
            .await
            .expect("create_in_progress should succeed");
        repository
            .append_segment(&TranscriptSegment::new(
                "session-1",
                "olá",
                Some("pt"),
                0,
                900,
            ))
            .await
            .expect("append_segment should succeed");

        repository
            .finalise("session-1", "olá", None, 900)
            .await
            .expect("finalise should succeed");

        let session = repository
            .get("session-1")
            .await
            .expect("get should succeed")
            .expect("session should exist");
        assert_eq!(session.language.as_deref(), Some("pt"));
    }

    /// Defensive: finalising a session whose header row is missing (a failed
    /// `create_in_progress`) must still persist it rather than silently
    /// dropping a finished recording.
    #[tokio::test]
    async fn test_finalise_inserts_when_the_session_row_is_missing() {
        let repository = SessionRepository::new(test_pool().await);

        repository
            .finalise("session-1", "hello", Some("en"), 1_000)
            .await
            .expect("finalise should succeed");

        let session = repository
            .get("session-1")
            .await
            .expect("get should succeed")
            .expect("session should exist");
        assert_eq!(session.transcript, "hello");
        assert_eq!(session.status, SESSION_STATUS_COMPLETE);
    }

    /// The DEC-009 crash scenario: a recording interrupted before
    /// `finalise` runs keeps every segment written up to that point, and
    /// stays distinguishable from a cleanly finished session.
    #[tokio::test]
    async fn test_interrupted_session_keeps_its_segments_and_stays_in_progress() {
        let repository = SessionRepository::new(test_pool().await);
        repository
            .create_in_progress("session-1")
            .await
            .expect("create_in_progress should succeed");
        for (text, start_ms) in [("first", 0), ("second", 1_000), ("third", 2_000)] {
            repository
                .append_segment(&TranscriptSegment::new(
                    "session-1",
                    text,
                    Some("en"),
                    start_ms,
                    start_ms + 900,
                ))
                .await
                .expect("append_segment should succeed");
        }
        // No `finalise` — this is where the process dies.

        let session = repository
            .get("session-1")
            .await
            .expect("get should succeed")
            .expect("interrupted session should still be there");
        assert_eq!(session.status, SESSION_STATUS_IN_PROGRESS);
        assert_eq!(session.transcript, "first second third");

        let segments = repository
            .segments("session-1")
            .await
            .expect("segments should be queryable");
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[2].text, "third");
    }

    #[tokio::test]
    async fn test_delete_removes_the_sessions_segments_too() {
        let pool = test_pool().await;
        let repository = SessionRepository::new(pool.clone());
        repository
            .create_in_progress("session-1")
            .await
            .expect("create_in_progress should succeed");
        repository
            .append_segment(&TranscriptSegment::new(
                "session-1",
                "hello",
                Some("en"),
                0,
                900,
            ))
            .await
            .expect("append_segment should succeed");

        repository
            .delete("session-1")
            .await
            .expect("delete should succeed");

        assert!(repository
            .segments("session-1")
            .await
            .expect("segments should be queryable")
            .is_empty());
    }

    /// Existing installs have a `sessions` table but no `_sqlx_migrations`
    /// table, so migration 0001 runs against a database that already has the
    /// table it creates. It must be a no-op there — and must not lose rows —
    /// rather than failing and leaving the app unable to start.
    #[tokio::test]
    async fn test_migrations_apply_to_a_pre_migration_database() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect");

        // The exact schema shipped before versioned migrations existed.
        sqlx::query(
            r#"
            CREATE TABLE sessions (
                id          TEXT PRIMARY KEY,
                created_at  TEXT NOT NULL,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                language    TEXT,
                transcript  TEXT NOT NULL,
                translation TEXT,
                target_lang TEXT,
                synced      INTEGER NOT NULL DEFAULT 0
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("legacy schema should be created");
        sqlx::query(
            "INSERT INTO sessions (id, created_at, duration_ms, transcript) VALUES ('old', '2026-01-01T00:00:00Z', 42, 'legacy transcript')",
        )
        .execute(&pool)
        .await
        .expect("legacy row should insert");

        crate::storage::db::run_migrations(&pool)
            .await
            .expect("migrations should apply to an existing database");

        let repository = SessionRepository::new(pool.clone());
        let session = repository
            .get("old")
            .await
            .expect("get should succeed")
            .expect("the pre-existing session should survive the migration");
        assert_eq!(session.transcript, "legacy transcript");
        assert_eq!(session.duration_ms, 42);
        // Sessions predating incremental writes were only ever written at
        // stop, so they're complete by definition.
        assert_eq!(session.status, SESSION_STATUS_COMPLETE);

        // And the new schema is fully in place.
        let count: i64 = sqlx::query("SELECT COUNT(*) AS count FROM segments")
            .fetch_one(&pool)
            .await
            .expect("segments table should exist")
            .get("count");
        assert_eq!(count, 0);
    }
}
