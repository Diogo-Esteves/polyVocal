<div align="center">

# 🎙️ PolyVocal

**Fast, local, cross-platform speech-to-text with translation.**

[![CI](https://github.com/diogoe/polyVocal/actions/workflows/ci.yml/badge.svg)](https://github.com/diogoe/polyVocal/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Built with Rust](https://img.shields.io/badge/Built%20with-Rust-orange?logo=rust)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8D8?logo=tauri)](https://tauri.app/)
![Status: Pre-release](https://img.shields.io/badge/Status-Pre--release-yellow)

*Speak in any language. Capture every word.*

</div>

---

PolyVocal is a productivity-focused speech-to-text app for professionals, developers, and enterprises. It transcribes your speech locally — no cloud, no accounts, no audio leaving your device — and translates the result on demand.

The name and logo draw on the playful image of someone singing into a hairbrush: expressive, immediate, uninhibited.

---

## ✨ Features

- 🎤 **Real-time transcription** — VAD-gated segments fed to Whisper for accurate, low-latency output
- 🌍 **Auto language detection** — speak English, Portuguese, Spanish (Hindi & Mandarin coming soon)
- 🔄 **On-demand translation** — local OPUS-MT models via `candle`, no network required
- 🤖 **Local models only (MVP)** — bring your own models from HuggingFace, no lock-in
- 🔒 **Privacy first** — audio never leaves your device; transcripts stay local
- 🖥️ **Cross-platform** — Linux, macOS, Windows
- ⌨️ **Global hotkey** — trigger recording from any app (opt-in)
- 🔁 **Sync-ready data model** — cross-device history sync coming in a future release

---

## 🏗️ Built With

| Layer | Technology |
|---|---|
| Desktop shell | [Tauri 2](https://tauri.app/) |
| Backend | Rust |
| Frontend | [Leptos](https://leptos.dev/) → WebAssembly |
| Transcription | [whisper-rs](https://github.com/tazz4843/whisper-rs) (`whisper.cpp`) |
| VAD | [Silero VAD](https://github.com/snakers4/silero-vad) via ONNX (`ort`) |
| Audio | `cpal` + `rubato` |
| Translation | OPUS-MT via [candle](https://github.com/huggingface/candle) |
| Storage | `sqlx` + SQLite |

Zero JavaScript. The entire stack — backend and frontend — is Rust.

---

## 🚀 Getting Started

> ⚠️ PolyVocal is in pre-release. Binaries are not yet available. Build from source below.

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Tauri CLI](https://tauri.app/start/prerequisites/) + system dependencies for your OS
- [`trunk`](https://trunkrs.dev/) — `cargo install trunk`
- WASM target — `rustup target add wasm32-unknown-unknown`

### Build & Run

```bash
git clone https://github.com/diogoe/polyVocal.git
cd polyVocal
cargo tauri dev
```

On first launch, PolyVocal downloads the `tiny` Whisper model (~75 MB). Larger models can be selected in Settings.

---

## 📦 Models

PolyVocal bundles the `tiny` Whisper model. You can download larger models or bring your own from HuggingFace.

| Model | Size | Notes |
|---|---|---|
| `tiny` | ~75 MB | Bundled — works immediately |
| `base` | ~145 MB | Recommended for most users |
| `small` | ~465 MB | Higher accuracy |
| `medium` | ~1.5 GB | Best accuracy |
| Custom | Any | Point to any GGUF-compatible model |

---

## 🗺️ Roadmap

| Phase | Focus | Status |
|---|---|---|
| 0 | Specification & architecture | ✅ Done |
| 1 | MVP — desktop transcription (Linux/macOS/Windows) | 🔨 In progress |
| 2 | Translation + model polish | 🔲 Planned |
| 3 | Cross-device sync | 🔲 Planned |
| 4 | Mobile (iOS/Android) | 🔲 Future |
| 5 | Plugins, export formats, API | 🔲 Future |

---

## 📄 Docs

| Document | Description |
|---|---|
| [SPEC.md](docs/SPEC.md) | Full product specification |
| [DECISIONS.md](docs/DECISIONS.md) | Architecture decision log (19 decisions) |
| [TECH_STACK.md](docs/TECH_STACK.md) | Stack analysis and rationale |
| [ROADMAP.md](docs/ROADMAP.md) | Phased delivery plan |
| [DESIGN.md](design/DESIGN.md) | UI/UX principles |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to contribute |

---

## 🤝 Contributing

Bug reports, language quality reports, and feature requests are the most valuable contributions right now. See [CONTRIBUTING.md](CONTRIBUTING.md) for details.

We have a dedicated issue template for **language accuracy reports** — if you're a native speaker of Portuguese, Spanish, Hindi, or Mandarin, your feedback is especially welcome.

---

## 📝 License

MIT © [Diogo Esteves](https://github.com/diogoe)

---

<div align="center">
<sub>AI-first project · Started 2026</sub>
</div>
