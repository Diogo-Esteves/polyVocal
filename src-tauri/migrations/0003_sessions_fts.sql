-- Full-text index over session transcripts (#209). External-content FTS5:
-- the indexed text lives only once, in sessions.transcript — this virtual
-- table stores just the inverted index. Kept in sync via triggers below
-- rather than touching every write path (create_in_progress, append_segment,
-- finalise, save, delete) individually. sessions.id is TEXT PRIMARY KEY, so
-- it doesn't alias SQLite's implicit rowid — that implicit rowid is what's
-- used here as the FTS content key.
CREATE VIRTUAL TABLE IF NOT EXISTS sessions_fts USING fts5(
    transcript,
    content='sessions',
    content_rowid='rowid'
);

-- Backfill existing rows — CREATE VIRTUAL TABLE starts empty.
INSERT INTO sessions_fts(rowid, transcript)
SELECT rowid, transcript FROM sessions;

CREATE TRIGGER IF NOT EXISTS sessions_fts_ai AFTER INSERT ON sessions BEGIN
    INSERT INTO sessions_fts(rowid, transcript) VALUES (new.rowid, new.transcript);
END;

CREATE TRIGGER IF NOT EXISTS sessions_fts_ad AFTER DELETE ON sessions BEGIN
    INSERT INTO sessions_fts(sessions_fts, rowid, transcript) VALUES ('delete', old.rowid, old.transcript);
END;

CREATE TRIGGER IF NOT EXISTS sessions_fts_au AFTER UPDATE ON sessions BEGIN
    INSERT INTO sessions_fts(sessions_fts, rowid, transcript) VALUES ('delete', old.rowid, old.transcript);
    INSERT INTO sessions_fts(rowid, transcript) VALUES (new.rowid, new.transcript);
END;
