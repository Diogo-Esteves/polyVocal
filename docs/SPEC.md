# PolyVocal — Product Specification

**Version:** 0.2 (post-architecture review)
**Status:** ✅ Architecture decided — ready for implementation
**Last updated:** 2026-07-28

---

## 1. Overview

PolyVocal is a fast, accurate, cross-platform speech-to-text application targeting professionals, developers, and enterprises who need to capture, transcribe, and translate spoken language as part of their productive workflows — note-taking, idea capture, meeting summaries, multilingual communication, and more.

It is **not** a daily-life communication tool. It is a focused productivity instrument.

The guiding analogy: like picking up a hairbrush to sing — expressive, immediate, uninhibited, and natural.

---

## 2. Goals

| Goal | Description |
|---|---|
| **Speed** | Transcription latency per utterance < 500ms on base model. |
| **Accuracy** | VAD-gated segments fed to Whisper — no word-boundary errors from chunking. |
| **Simplicity** | One primary action: press a button and speak. No learning curve. |
| **Multilingual** | Auto-detects spoken language. No manual selection required. |
| **Translation** | On-demand, local, post-processing — click to translate any session. |
| **Portability** | Works fully offline. No cloud dependency in MVP. |
| **Sync** | Transcript history synced across devices (post-MVP, foundation laid in MVP). |
| **Privacy** | Audio and transcripts never leave the device without explicit user action. |

---

## 3. Non-Goals (MVP)

- ❌ Audio/video file import (post-MVP)
- ❌ Real-time collaborative transcription (post-MVP)
- ❌ Cloud-based transcription (post-MVP)
- ❌ Web application
- ❌ Mobile apps (post-MVP)
- ❌ User authentication / accounts (post-MVP)
- ❌ Speaker diarisation (post-MVP)
- ❌ E2E automated UI tests (post-MVP)

---

## 4. Target Users

### Primary
- **Professionals** — capturing meeting notes, ideas, voice memos
- **Developers** — using PolyVocal directly or integrating via future API/plugin
- **Enterprises** — productivity tooling for multilingual teams

### Secondary (future)
- Content creators needing transcripts
- Researchers and journalists
- Students in multilingual environments

---

## 5. Supported Platforms

### MVP
- 🖥️ Linux (x86_64, ARM64)
- 🍎 macOS (Intel + Apple Silicon)
- 🪟 Windows 10/11 (x86_64)

### Post-MVP
- 📱 iOS / Android
- 🔌 Editor plugins (VS Code, Zed, etc.)

---

## 6. Languages

### Transcription (MVP)
- English 🇬🇧
- Portuguese 🇧🇷🇵🇹
- Spanish 🇪🇸

### Transcription (planned)
- Hindi 🇮🇳
- Mandarin 🇨🇳
- French, German, Italian, Japanese, Korean

> Language is **auto-detected** per utterance. Users never select a language before speaking.

### Translation (MVP)
- `en ↔ pt`, `en ↔ es`, `pt ↔ es` via OPUS-MT / `candle`
- Source: any detected language
- Target: user-selected in UI

### Translation (planned)
- Additional language pairs as OPUS-MT models are added

---

## 7. Architecture

### 7.1 Technology Stack

| Layer | Technology |
|---|---|
| Desktop shell | Tauri 2 |
| Backend language | Rust (sole language of record) |
| Frontend language | Rust → WASM via Leptos |
| Frontend build | `trunk` |
| Tauri IPC (WASM) | `tauri-sys` crate |
| Transcription | `whisper-rs` (bindings over `whisper.cpp`) |
| VAD | Silero via ONNX Runtime (`ort` crate) |
| Audio resampling | `rubato` crate |
| Audio capture | `cpal` crate |
| Translation | OPUS-MT via HuggingFace `candle` |
| Storage | `sqlx` + SQLite |
| Async runtime | Tokio |
| Config | `serde` + `toml` |
| Logging | `tracing` + `tracing-subscriber` |
| Auto-update | `tauri-plugin-updater` |
| Global hotkey | `tauri-plugin-global-shortcut` |

### 7.2 Audio Pipeline

```
[Dedicated OS thread]
  cpal (raw format, any sample rate)
    → rubato (resample to 16 kHz mono f32, unconditionally)
    → mpsc channel
        ↓
[Tokio spawn_blocking]
  Silero VAD (ONNX)
    → speech segment detected
        ↓
[Tokio spawn_blocking]
  whisper-rs inference
    → TranscriptSegment { text, language, start_ms, end_ms }
        ↓
[Tokio async]
  Tauri emit("transcript:segment", payload)
    → Leptos UI (listen via tauri-sys)
    → SQLite (incremental write)
```

### 7.3 Threading Model

- **Dedicated OS thread** — audio capture (`cpal`). Low jitter, never blocked by the async scheduler.
- **Tokio `spawn_blocking`** — VAD inference and Whisper inference. CPU-bound, off the event loop.
- **Tokio async** — orchestration, IPC, storage writes, event emission.
- **`std::sync::mpsc` channel** — bridges the OS audio thread to the Tokio runtime.

### 7.4 App State

Per-domain state, each registered via Tauri `manage()`:

| State | Type | Contents |
|---|---|---|
| `AudioState` | `Arc<Mutex<AudioState>>` | Is recording? Active device. |
| `SessionState` | `Arc<Mutex<SessionState>>` | Current session ID, partial transcript. |
| `ModelState` | `Arc<Mutex<ModelState>>` | Active Whisper model path, active OPUS-MT pair. |

### 7.5 Frontend Architecture

- **Leptos** with signals-based reactivity
- Compiled to WASM, runs inside Tauri's WebView
- Communicates with Rust backend via:
  - `invoke()` (commands) — user-initiated actions
  - `listen()` (events) — real-time transcript updates pushed from Rust

---

## 8. Core Features

### 8.1 Live Transcription

- Single button (or keyboard shortcut) to start/stop
- Audio captured from selected microphone
- Silero VAD detects utterance boundaries
- Each utterance sent to Whisper for inference
- Transcript segments emitted to UI in real time via Tauri events
- Language auto-detected per segment

**Interaction modes (user-configurable in Settings):**
- **Hold-to-talk** — hold button/key, release to stop
- **Toggle** — press once to start, press again to stop

**Keyboard shortcut:**
- In-app shortcut: always available
- Global hotkey: opt-in in Settings, triggers OS permission request

### 8.2 Translation

- Post-processing — triggered by user action on a completed session
- Target language selected via language picker
- Runs locally via OPUS-MT + `candle` — no network request
- Both original and translated text preserved and displayed
- Translation stored back to the session record in SQLite

### 8.3 Transcript History

- All sessions stored locally in SQLite
- Incremental writes per segment during recording; finalised on stop
- Each session: ID, device ID, timestamp, duration, language, transcript, translation
- Searchable by keyword, date, language
- Exportable as plain text, Markdown, JSON (Pro)

### 8.4 Model Management

- Bundled at install: `ggml-tiny.bin` + `en↔pt` OPUS-MT pair
- Settings panel: download `base`, `small`, `medium` Whisper models
- **Bring-your-own models** — point PolyVocal at any compatible GGUF model from HuggingFace or other sources
- Model download: streaming with progress indicator, resumable

### 8.5 Cross-Device Sync (post-MVP, foundations in MVP)

**MVP data model (sync-ready):**
- Each session and segment has a UUID v4
- Each record carries `device_id` (which device created it)
- `synced: bool` flag tracks push status
- Wire format documented in DEC-015

**Post-MVP sync:**
- Transcript history synced across devices
- No account required — device-pairing / token-based
- Conflict resolution: last-write-wins (CRDT upgrade path available)

---

## 9. Settings

All settings persisted in `config.toml` in the platform app data directory.

| Setting | Default | Notes |
|---|---|---|
| Active Whisper model | `tiny` | Selectable from downloaded models |
| Microphone | System default | Any available input device |
| Interaction mode | Toggle | Toggle or hold-to-talk |
| In-app shortcut | `Space` | Configurable |
| Global hotkey | Disabled | Opt-in, requires OS permission |
| Default translation target | `en` | Any supported language |
| Crash reporting | Prompt on first launch | Opt-in |
| Models directory | App data dir | Can be changed (for large model storage) |

---

## 10. Model Architecture

### Whisper Models

| Model | Size | Speed | Accuracy | Bundled |
|---|---|---|---|---|
| tiny | ~75 MB | ⚡⚡⚡ | ★★☆ | ✅ Yes |
| base | ~145 MB | ⚡⚡ | ★★★ | Download |
| small | ~465 MB | ⚡ | ★★★★ | Download |
| medium | ~1.5 GB | 🐢 | ★★★★★ | Download |
| custom | Any | Varies | Varies | Bring-your-own |

### OPUS-MT Translation Models

| Pair | Size | Bundled |
|---|---|---|
| en ↔ pt | ~1 MB | ✅ Yes |
| en ↔ es | ~1 MB | Download |
| pt ↔ es | ~1 MB | Download |
| + future pairs | ~1 MB each | Download |

---

## 11. Data & Privacy

### Local Storage
- SQLite database in platform app data directory
- Audio is **never stored** — transcription only
- Transcripts: plaintext + metadata in SQLite
- Models: managed directory, configurable path

### Privacy Guarantees
- All transcription and translation runs **on-device**
- No audio, transcript, or file path data transmitted without explicit user action
- Crash reports (opt-in): stack traces and system info only — **transcript content never included**
- No usage analytics, ever

### Sync (post-MVP)
- End-to-end encrypted in transit
- No plaintext transcript on any server
- Explicit user consent required

---

## 12. Error Handling

Three tiers, applied per error class:

| Tier | When | UI |
|---|---|---|
| **Silent** | Transient errors (dropped chunk, VAD miss) | Log only, auto-retry |
| **Toast** | Recoverable failures (download failed, translation unavailable) | Non-blocking notification, dismissible |
| **Dialog** | Critical startup failures (no mic, no model, DB corrupt) | Blocking modal, requires user action |

---

## 13. Performance Targets

| Metric | Target |
|---|---|
| Utterance-to-text latency (tiny model) | < 500ms |
| Utterance-to-text latency (base model) | < 1s |
| App cold start | < 2s |
| Model load time (base) | < 3s |
| Memory usage (base model active) | < 400 MB |
| Installer size | ~80 MB (tiny model bundled) |
| Binary size (excl. models) | < 50 MB |

---

## 14. Monetisation

**Model: Freemium + Open Core**

| Tier | Price | Features |
|---|---|---|
| **Free** | $0 | `tiny` model, EN/PT/ES transcription, local storage, bring-your-own models |
| **Pro** | TBD subscription | Sync, translation, all future value-add features |
| **Enterprise** | TBD | Volume, self-hosted sync, API access, priority support |

**Key principle:** The transcription engine is **never locked**. Free users can bring any model from HuggingFace. Pro value is in services (sync, translation), not in crippling the free tier.

**Open source strategy (TBD):**
- Core transcription engine + CLI → open source (MIT or Apache 2.0)
- Desktop app → potentially open source
- Sync backend + Pro features → proprietary or source-available

---

## 15. Testing Strategy

- **Unit tests** — all pure Rust logic: resampler, VAD, session repository, config, model registry
- **Integration tests** — audio pipeline with known fixtures: feed audio in multiple languages, assert transcript quality
- **Fixtures** — `fixtures/` directory of known audio samples, maintained in the repo
- **E2E** — deferred; developer uses the app daily and files bugs

---

## 16. Auto-Update

- **Tauri updater plugin** — checks GitHub Releases manifest on launch
- **Non-blocking notification** — user chooses when to update, never forced
- **Package manager support** (Homebrew, Flatpak, winget) — Phase 3+

---

## 17. Open Questions

| # | Question | Status |
|---|---|---|
| OQ-01 | Sync mechanism transport (iCloud Drive / Google Drive file sync vs custom P2P vs server?) | 🔲 Post-MVP |
| OQ-02 | Open source boundary — what exactly is MIT vs proprietary? | 🔲 Phase 2 |
| OQ-03 | Pro licence check implementation (local licence file vs online validation?) | 🔲 Phase 2 |
| OQ-04 | Real-time collaboration concept — what does it actually look like? | 🔲 Future |
| OQ-05 | Mobile strategy — Tauri Mobile vs separate native apps? | 🔲 Phase 4 |

---

## 18. Out of Scope (explicitly)

- Browser / web app
- Daily casual communication (messaging, voice notes between people)
- Live captioning for video calls (future plugin opportunity)
- Cloud transcription at launch
- Usage analytics or behavioural tracking of any kind
