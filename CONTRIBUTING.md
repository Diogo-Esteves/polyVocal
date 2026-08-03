# Contributing to PolyVocal

Thanks for your interest in contributing! PolyVocal is an early-stage project — the best contributions right now are bug reports, language testing, and feedback on the core experience.

---

## Ways to Contribute

### 🐛 Bug Reports
Open an issue using the **Bug Report** template. Include your OS, model size, and the language you were speaking. The more specific, the better.

### 💡 Feature Requests
Open an issue using the **Feature Request** template. Check existing issues first to avoid duplicates.

### 🌍 Language Testing
PolyVocal targets multiple languages. If you speak Portuguese, Spanish, Hindi, or Mandarin natively, testing transcription accuracy and opening issues with examples is incredibly valuable.

### 🔧 Code Contributions
At this stage the codebase is moving fast. Before opening a PR:
1. Open an issue describing what you want to change and why
2. Wait for acknowledgement before investing time in implementation
3. Keep PRs focused — one concern per PR

---

## Development Setup

### Prerequisites

- Rust (stable, via `rustup`) — `rustup update stable`
- Node.js (for Tauri CLI) — `npm install -g @tauri-apps/cli`
- `trunk` (Leptos/WASM build tool) — `cargo install trunk`
- `wasm32-unknown-unknown` target — `rustup target add wasm32-unknown-unknown`
- Tauri system dependencies — see [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS
- Docker (optional — only needed to run LibreTranslate locally for translation work; see README's [Translation](README.md#-translation) section)

### Running in development

```bash
# Clone the repo
git clone https://github.com/diogoe/polyVocal.git
cd polyVocal

# Run the app in dev mode
cargo tauri dev
```

---

## Code Style

- **Rust:** standard `rustfmt` formatting — run `cargo fmt` before committing
- **No JavaScript** — the frontend is Leptos/WASM, keep it that way
- **Error handling:** use `anyhow` for application errors, `thiserror` for library errors
- **Logging:** use `tracing` macros (`info!`, `debug!`, `warn!`, `error!`) — no `println!`

---

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add hold-to-talk mode
fix: correct resampler channel count on Linux
docs: update model download instructions
chore: bump whisper-rs to 0.12
```

---

## Code of Conduct

Be kind. Be direct. Focus on the work.
