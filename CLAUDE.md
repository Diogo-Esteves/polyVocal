# CLAUDE.md

Guidance for Claude Code (or any coding agent) working in this repo.

## What this project is

PolyVocal is a local, privacy-first speech-to-text + translation desktop app.
Tauri 2 (Rust) shell, Leptos/WASM frontend, zero JavaScript. Transcription
via `whisper-rs` + Silero VAD (`ort`), translation via local OPUS-MT models
run through `candle` — no cloud, no sidecar process. See the root
`README.md` for the full feature/stack overview.

## Repo layout

Two independent Rust crates, no top-level Cargo workspace:

- `src-tauri/` — the backend (`polyvocal_lib` / `polyvocal` binary). Tauri
  commands, audio capture, transcription pipeline, translation engine,
  SQLite storage. This is where almost all logic and tests live.
- `src/` — the frontend (`polyvocal-ui`), Leptos compiled to WASM via
  `trunk`. Talks to the backend only through `tauri_sys::core::invoke*`
  calls and Tauri events (`transcript:segment`, etc.) — no JS.
- `docs/ROADMAP.md`, `docs/TECH_STACK.md`, `design/DESIGN.md` — public,
  tracked docs.
- `docs/DECISIONS.md`, `docs/SPEC.md` — **internal-only, gitignored.**
  Deep architecture-decision log and full spec. Not present in a fresh
  clone; if they exist on your checkout, they're the deepest source of
  truth for *why* things are built the way they are — read them before
  touching translation, audio, or storage internals.

## Commands

Run from the crate's own directory (`src-tauri/` or `src/`), not the repo root.

```bash
# Backend (src-tauri/)
cargo check
cargo fmt --all -- --check        # cargo fmt --all to fix
cargo clippy --all-targets -- -D warnings
cargo test --all                  # unit tests + tests/pipeline_integration.rs

# Frontend (src/)
cargo fmt --all -- --check
cargo clippy --target wasm32-unknown-unknown --all-targets -- -D warnings
trunk build                       # or `trunk serve` for dev

# Whole app
cargo tauri dev                   # from repo root
```

CI (`.github/workflows/ci.yml`) runs all of the above across Linux/macOS/
Windows and is the ground truth if this file drifts — check it if unsure.

A handful of `#[ignore]`d tests in `src-tauri` (real Whisper/Silero/OPUS-MT
inference, tagged e.g. `test_real_candle_translation_*`) download real
model weights (hundreds of MB) on first run. They're not part of `cargo
test --all`'s default set; run with `cargo test -- --ignored` only when
you've touched engine/tokenizer/model-registry wiring and want to verify
against real weights.

## Testing conventions already established — follow them, don't reinvent

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

## Code style (from `CONTRIBUTING.md` — repeated here since agents don't always read that file)

- Standard `rustfmt`; `cargo fmt` before committing.
- `anyhow` for application errors, `thiserror` for library errors.
- `tracing` macros (`info!`, `debug!`, `warn!`, `error!`) — never `println!`.
- No JavaScript, ever — frontend changes stay in Leptos/Rust.
- Commit messages: [Conventional Commits](https://www.conventionalcommits.org/)
  (`feat:`, `fix:`, `docs:`, `chore:`, `ci:`).

## Finding out what to work on next

`docs/ROADMAP.md` is the checklist of phases/features, but **checkboxes can
go stale** — a past session found "Language auto-detection" marked
unchecked when it was in fact fully implemented end-to-end. Before treating
an unchecked roadmap item as real work, grep the code for it first; before
reporting one as done, verify the roadmap text still matches reality.

## Working style notes

- Prefer direct, in-place edits for normal-sized changes (a handful of
  files, one concern) over spinning up a separate worktree — reserve
  worktrees/parallel agents for genuinely independent, large-scope work.
- Feature branches + PR, rebase-and-merge on GitHub (branch names so far:
  `feat/...`, `chore/...`; PR merges rewrite commit hashes on `main` — if a
  stacked branch stops appearing merged after another PR lands, that's why,
  not a sign work was lost).
- Run `cargo fmt`, `cargo clippy -D warnings`, and the full test suite
  before calling backend work done — all three are cheap and CI enforces
  them anyway.
