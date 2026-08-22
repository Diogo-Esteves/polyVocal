---
description: Run fmt, clippy, and tests for both crates before calling backend/frontend work done
allowed-tools: Bash(cargo fmt:*), Bash(cargo clippy:*), Bash(cargo test:*), Bash(trunk build:*)
---

Run the full pre-commit check set from `.claude/rules/commands.md` for
whichever crate(s) this session actually touched — don't run the frontend
checks for a backend-only diff or vice versa. Report pass/fail per command,
don't just say "done."

Backend (run from `src-tauri/`):

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

Frontend (run from `src/`):

```bash
cargo fmt --all -- --check
cargo clippy --target wasm32-unknown-unknown --all-targets -- -D warnings
trunk build
```

If `cargo fmt --check` fails, run `cargo fmt --all` (no `--check`) to fix
it, then re-run the check to confirm. If `cargo test --all` is being run
for the first time in this checkout/worktree, expect it to need network —
see the `cargo test --all` network caveat in `.claude/rules/commands.md`
before treating a slow first run as a hang.
