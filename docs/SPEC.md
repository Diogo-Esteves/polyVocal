# PolyVocal — Product Specification

**Version:** 0.1 (pre-development draft)  
**Status:** 🔲 Draft — pending tech stack decision  
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
| **Speed** | Transcription latency must feel near-instant. Target: first words appear within ~500ms of speaking. |
| **Accuracy** | Word error rate competitive with best-in-class local models (Whisper-class). |
| **Simplicity** | One primary action: press a button and speak. No learning curve. |
| **Multilingual** | Recognise and transcribe multiple spoken languages without manual selection. |
| **Translation** | Allow on-demand translation of any transcript into a target language. |
| **Portability** | Works offline. No cloud dependency required. |
| **Sync** | Transcript history available across the user's devices. |

---

## 3. Non-Goals (MVP)

- ❌ Real-time audio/video file import (post-MVP)
- ❌ Real-time collaborative transcription (post-MVP)
- ❌ Cloud-based transcription (post-MVP)
- ❌ Web application
- ❌ Mobile apps (post-MVP)
- ❌ User authentication / accounts (post-MVP)
- ❌ Speaker diarisation (post-MVP)

---

## 4. Target Users

### Primary
- **Professionals** — capturing meeting notes, ideas, voice memos
- **Developers** — using it as a tool or integrating it via API/plugin
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
- 📱 iOS
- 🤖 Android
- 🔌 Editor plugins (VS Code, Zed, etc.)

---

## 6. Languages

### Transcription (MVP)
- English 🇬🇧
- Portuguese 🇧🇷🇵🇹
- Spanish 🇪🇸🇲🇽

### Transcription (planned expansion)
- Hindi 🇮🇳
- Mandarin 🇨🇳
- French, German, Italian, Japanese, Korean (as model coverage allows)

> **Note:** Language detection is automatic. Users do not need to select the language before speaking. The system infers it from audio.

### Translation (MVP)
- Source: any detected language
- Target: English, Portuguese, Spanish (selectable)

### Translation (planned expansion)
- Any language supported by the translation backend (LibreTranslate / Argos or similar open-source engine)

---

## 7. Core Features

### 7.1 Live Transcription

- Activated by a single button (or keyboard shortcut)
- Audio is captured from the default system microphone (configurable)
- Text appears in real time as speech is processed
- Language is auto-detected per session (or per utterance, TBD)
- Session ends when the user stops recording

**Interaction modes (user-configurable):**
- **Hold to talk** — hold button/key, release to stop
- **Toggle** — press once to start, press again to stop

### 7.2 Translation

- Available as a post-processing action on any completed transcript
- User taps/clicks a "Translate" button and selects a target language
- Translation is performed locally (MVP) or via a cloud API (post-MVP)
- Both original and translated text are preserved and viewable

### 7.3 Transcript History

- All sessions are stored locally in a structured database
- Each entry contains: timestamp, duration, detected language, raw transcript, translation (if requested)
- Searchable by keyword, date, and language
- Exportable as plain text, Markdown, or JSON

### 7.4 Cross-Device Sync (post-MVP foundations laid in MVP)

- MVP: local storage only, with a sync-ready data model
- Post-MVP: transcript history synced across devices via an encrypted sync layer (self-hosted or provider TBD)
- No account required for MVP — sync will use a device-pairing or token-based mechanism (no email/password login)

---

## 8. Model Architecture

### Transcription Engine (MVP)

- **Local only** — no data leaves the device
- Primary target: **Whisper-compatible models** (e.g. `whisper.cpp`, `faster-whisper`, `whisper-rs`)
- Model sizes available: `tiny`, `base`, `small`, `medium` — user selects based on hardware
- Default: `base` (balance of speed and accuracy on modern hardware)

### Model Selection

Users can switch models in Settings:

| Model | Size | Speed | Accuracy | Recommended for |
|---|---|---|---|---|
| tiny | ~75 MB | ⚡⚡⚡ | ★★☆ | Low-power devices, quick notes |
| base | ~145 MB | ⚡⚡ | ★★★ | Default — most users |
| small | ~465 MB | ⚡ | ★★★★ | Better accuracy, decent hardware |
| medium | ~1.5 GB | 🐢 | ★★★★★ | High accuracy, powerful machines |

### Translation Engine (MVP)

- Local translation: **LibreTranslate** or **Argos Translate** (open-source, offline-capable)
- MVP scope: EN ↔ PT ↔ ES
- Future: cloud translation (DeepL, Google Translate) as optional upgrade

### Cloud Models (post-MVP)

- OpenAI Whisper API
- Deepgram
- AssemblyAI
- Selection via Settings; API key stored encrypted locally

---

## 9. UI/UX

### Design Principles

1. **One primary action** — the record button dominates the screen
2. **Immediate feedback** — waveform animation starts the moment the mic is active
3. **Text first** — transcript is the hero content, not controls
4. **Language-neutral** — UI chrome works regardless of transcript language direction (LTR/RTL future consideration)

### Key Screens

#### Home / Record
- Large centred record button (hairbrush-inspired icon)
- Live waveform visualisation when active
- Detected language badge
- Scrolling live transcript below
- Recent sessions listed underneath (collapsed)

#### Transcript Detail
- Full transcript text
- Original language + detected language shown
- "Translate" button → language picker → translated text shown inline
- Copy, Export, Delete actions
- Timestamp and duration metadata

#### Settings
- Microphone selector
- Model selector (with download progress for first use)
- Interaction mode (hold vs toggle)
- Default translation target language
- Storage location
- (Post-MVP) Sync configuration

---

## 10. Data & Storage

### Local Storage
- Database: embedded (SQLite or equivalent) — no external server required
- Audio: **not stored** by default (transcription only) — opt-in audio archive as future feature
- Transcripts: plain text + metadata, stored in app data directory
- Models: downloaded on first use to a `models/` directory, managed by the app

### Sync Data Model (designed in MVP, activated post-MVP)
- Each transcript has a UUID
- Conflict resolution: last-write-wins (simple), upgradeable to CRDT (future)
- Sync transport: TBD (could be iCloud/Drive as file sync, or a custom P2P/server layer)

---

## 11. Performance Targets

| Metric | Target |
|---|---|
| Time to first transcribed word | < 500ms |
| Sustained transcription lag | < 1s behind speech |
| App cold start | < 2s |
| Model load time (base) | < 3s |
| Memory usage (base model) | < 400 MB |
| Binary size | < 50 MB (excl. models) |

---

## 12. Monetisation

**Model: Freemium + Open Core**

| Tier | Price | Features |
|---|---|---|
| **Free** | $0 | Local transcription, base/tiny models, 3 languages, no sync |
| **Pro** | TBD (subscription) | All models, all languages, sync, translation, export formats |
| **Enterprise** | TBD | Volume, self-hosted sync, API access, priority support |

**Open source strategy (TBD):**
- Core transcription engine and CLI → open source (MIT or Apache 2.0)
- Desktop app shell → potentially open source
- Sync backend and Pro features → proprietary or source-available

---

## 13. Security & Privacy

- All transcription happens **on-device** (MVP)
- No audio data transmitted without explicit user action
- Local database is unencrypted by default (opt-in encryption post-MVP)
- When cloud features are added: E2E encryption for sync, explicit consent for audio uploads

---

## 14. Open Questions

| # | Question | Owner | Status |
|---|---|---|---|
| OQ-01 | Tech stack decision (see TECH_STACK.md) | Team | 🔲 Open |
| OQ-02 | Translation engine: LibreTranslate vs Argos Translate vs other? | Team | 🔲 Open |
| OQ-03 | Sync mechanism design (file-based vs custom server vs P2P?) | Team | 🔲 Open |
| OQ-04 | Open source boundary — what is free vs proprietary? | Team | 🔲 Open |
| OQ-05 | Whisper model download UX — bundled or downloaded on demand? | Team | 🔲 Open |
| OQ-06 | Real-time collaboration concept — what does it look like? | Team | 🔲 Future |

---

## 15. Out of Scope (explicitly)

- Browser / web app
- Daily casual communication (messaging, voice notes between people)
- Live captioning for video calls (future plugin opportunity)
- Paid cloud transcription at launch
