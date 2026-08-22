# Code style

(From `CONTRIBUTING.md` — repeated here since agents don't always read that file.)

- Standard `rustfmt`; `cargo fmt` before committing.
- `anyhow` for application errors, `thiserror` for library errors.
- `tracing` macros (`info!`, `debug!`, `warn!`, `error!`) — never `println!`.
- No JavaScript, ever — frontend changes stay in Leptos/Rust.
- Commit messages: [Conventional Commits](https://www.conventionalcommits.org/)
  (`feat:`, `fix:`, `docs:`, `chore:`, `ci:`).
- Default to no comments. Only write one when the *why* is non-obvious — a
  hidden constraint, a workaround, a subtle invariant — not what the code
  already says through naming.
- Three similar lines beat a premature abstraction. Don't add
  configurability, error handling, or fallbacks for cases that can't
  currently happen.
