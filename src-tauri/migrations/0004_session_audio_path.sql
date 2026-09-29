-- Optional retained-audio file path per session (#191 first slice: capture
-- + storage only — no player/correction UI yet, see #213/#214/#215). NULL
-- for every session unless the user has opted in via Settings' "Save
-- recording audio" toggle (AppConfig::retain_audio, default off) — this
-- app is privacy-first, so nothing is written to disk unless asked for.
ALTER TABLE sessions ADD COLUMN audio_path TEXT;
