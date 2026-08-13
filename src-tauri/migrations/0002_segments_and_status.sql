-- Incremental segment persistence (DEC-009).
--
-- `segments` holds one row per VAD-closed speech segment, written as it is
-- transcribed rather than at stop, so a crash mid-recording loses at most the
-- last utterance. `start_ms`/`end_ms` are offsets from the start of the
-- recording (not whisper's per-chunk timestamps), which is what SRT export
-- needs.
CREATE TABLE IF NOT EXISTS segments (
    id         TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    start_ms   INTEGER NOT NULL,
    end_ms     INTEGER NOT NULL,
    text       TEXT NOT NULL,
    language   TEXT
);

CREATE INDEX IF NOT EXISTS idx_segments_session_start
    ON segments (session_id, start_ms);

-- `in_progress` until `stop_recording` finalises the session; a row still
-- marked `in_progress` at launch is one that was interrupted (crash, OOM,
-- force-quit) and never finalised. Existing rows all predate incremental
-- writes — they were only ever written at stop — so `complete` is the
-- correct default for them.
ALTER TABLE sessions ADD COLUMN status TEXT NOT NULL DEFAULT 'complete';
