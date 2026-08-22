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

## Rules

Conventions are split by topic in `.claude/rules/` — read the one(s)
relevant to what you're touching rather than all of them up front:

- `.claude/rules/commands.md` — build/lint/test commands, and the
  `cargo test --all` network caveat. Read this before running any check.
- `.claude/rules/testing.md` — test-pool and trait-seam-mocking patterns.
  Read before adding or changing tests.
- `.claude/rules/code-style.md` — formatting, error-handling, and comment
  conventions.
- `.claude/rules/workflow.md` — how work gets scoped against
  `docs/ROADMAP.md`/GitHub issues, when to hand off to the `implementer`
  or `chore` subagent, and branch/PR conventions.

## Local environment

Machine-specific setup — GUI availability, worktree build-cache tricks,
headless-QA tooling paths, this checkout's Orca worktree conventions —
lives in `CLAUDE.local.md` at the repo root if present. It's gitignored
and not shared; if it's missing on your checkout there's nothing to read.

## Custom commands

`.claude/commands/` has packaged workflows for recurring tasks — see
`/precommit`, `/roadmap-check`, and `/triage-issue`.
