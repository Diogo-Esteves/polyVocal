# Build, lint, and test commands

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

## `cargo test --all` needs network — this is not a hermetic suite

`src-tauri/tests/pipeline_integration.rs` is **not** `#[ignore]`d. It runs
as part of the default `cargo test --all`, downloads real Whisper (tiny) +
Silero weights on first run, and then runs real inference over
`fixtures/jfk.wav` (deliberate — see DEC-019 in its header). Expect the
first `cargo test --all` in a fresh checkout/worktree to need network and
take a few minutes. Grep `src-tauri/tests/`, not just `src-tauri/src/`,
before changing any pipeline/segmenter/engine signature — this file calls
those APIs and a `src/`-only grep will miss it.

A separate handful of `#[ignore]`d tests (real Whisper/Silero/OPUS-MT
inference beyond the one fixture above, tagged e.g.
`test_real_candle_translation_*`) also download model weights but are
genuinely opt-in: run with `cargo test -- --ignored` only when you've
touched engine/tokenizer/model-registry wiring and want to verify against
real weights.
