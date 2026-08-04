# PolyVocal — Roadmap

> Phases will be refined once the full spec is complete.

---

## Phase 0 — Foundation
- [x] Project scaffolding
- [x] SPEC.md completed
- [x] Technology stack decided
- [x] Design system defined *(logo + palette + typography (OS-native font stack) + icon set (Lucide, inlined SVG) + dark/light mode (`prefers-color-scheme` + manual override) — see `design/DESIGN.md`)*

## Phase 1 — MVP (Desktop) *(current)*
- [x] Audio capture from microphone
- [x] Transcription via local Whisper model *(real whisper-rs + Silero VAD inference wired end-to-end; models downloaded on demand via HuggingFace, not bundled — proven by `tests/pipeline_integration.rs` against `fixtures/jfk.wav`)*
- [x] Language auto-detection *(engine detects and persists the language per session; UI shows it live off `transcript:segment`, and `translate_text` uses the persisted value with local-detection fallback per DEC-010 — see `src/src/main.rs` and `commands/translation.rs`)*
- [x] Basic UI (single screen) *(Leptos + WASM app in `src/`, built via `trunk`; record/stop, live transcript via the `transcript:segment` event, detected language, and a translate action wired to `start_recording`/`stop_recording`/`translate_text` — see `src/src/main.rs`)*
- [x] Linux + macOS + Windows support *(CI now matrixes `check` and `test` jobs across ubuntu-latest, macos-latest, windows-latest, including the real-model `pipeline_integration.rs` test on all three)*

## Phase 2 — Translation & Polish
- [x] Text translation integration *(`translate_text` command wired to a local OPUS-MT/`candle` engine per DEC-010 — no sidecar process; uses the session's detected source language, falling back to local language detection, persists the translation, target language is caller-supplied — proven by `commands::translation` unit tests and manual `--ignored` end-to-end tests against real downloaded OPUS-MT weights)*
- [ ] Model switcher (local ↔ cloud) *(local Whisper size switcher — tiny/base/small/medium — exists; no cloud provider integration)*
- [ ] Improved accuracy and latency

## Phase 3 — Sync & Multi-device
- [ ] Account / identity layer
- [ ] Cross-device transcript sync
- [ ] Session history

## Phase 4 — Mobile
- [ ] iOS app
- [ ] Android app

## Phase 5 — Advanced Features
- [ ] Speaker diarisation
- [ ] Export formats (SRT, TXT, DOCX)
- [ ] API / integrations

---

*Dates TBD after spec is finalised.*
