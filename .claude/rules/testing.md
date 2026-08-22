# Testing conventions already established — follow them, don't reinvent

- **In-memory SQLite per test**: a local `test_pool()` helper (duplicated
  per test module, not shared — see `commands/translation.rs` and
  `storage/repository.rs`) spins up `sqlite::memory:` with
  `max_connections(1)` (a pool > 1 connection sees empty tables — each
  connection gets its own in-memory DB) and runs `storage::db::run_migrations`.
- **Trait-seam mocking for engines**: business logic that calls into real
  ML inference (translation, transcription) is split into a small `trait`
  (e.g. `Translator` in `commands/translation.rs`) so the orchestration
  logic — source-language fallback, persistence, error handling — is
  unit-tested (in a plain fn like `translate_session`) against a fake
  implementation, while the real engine is exercised only by the
  `#[ignore]`d end-to-end tests. Follow this pattern for any new engine
  integration rather than mocking at the HTTP/FFI layer.
- Tauri `#[tauri::command]` functions should stay thin — resolve state,
  delegate to a plain, independently-testable async fn or a repository/
  engine method. Don't put logic directly in the command body if it needs
  test coverage.

See `commands.md` for what `cargo test --all` actually runs (it needs
network — it is not a hermetic default suite, despite what you'd expect).
