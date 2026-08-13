-- Baseline schema: the `sessions` table exactly as `run_migrations`
-- hand-rolled it before versioned migrations existed.
--
-- Deliberately `IF NOT EXISTS`: databases created by earlier builds already
-- have this table but no `_sqlx_migrations` row proving it, so sqlx will run
-- this file against them on first launch after upgrading. It has to be a
-- no-op there, and create the table on a fresh install.
CREATE TABLE IF NOT EXISTS sessions (
    id          TEXT PRIMARY KEY,
    created_at  TEXT NOT NULL,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    language    TEXT,
    transcript  TEXT NOT NULL,
    translation TEXT,
    target_lang TEXT,
    synced      INTEGER NOT NULL DEFAULT 0
);
