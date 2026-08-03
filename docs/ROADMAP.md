# PolyVocal — Roadmap

> Phases will be refined once the full spec is complete.

---

## Phase 0 — Foundation
- [x] Project scaffolding
- [x] SPEC.md completed
- [x] Technology stack decided
- [ ] Design system defined *(logo + palette only — typography, icons, dark/light mode still open)*

## Phase 1 — MVP (Desktop) *(current)*
- [x] Audio capture from microphone
- [x] Transcription via local Whisper model *(real whisper-rs + Silero VAD inference wired end-to-end; models downloaded on demand via HuggingFace, not bundled — proven by `tests/pipeline_integration.rs` against `fixtures/jfk.wav`)*
- [ ] Language auto-detection *(engine detects and persists the language per session; nothing consumes it yet — no UI, no downstream logic)*
- [ ] Basic UI (single screen) *(`src/` is still empty — frontend framework decision pending, no commands wired to any UI)*
- [ ] Linux + macOS + Windows support *(build config is cross-platform; CI only runs on Ubuntu — no macOS/Windows coverage yet)*

## Phase 2 — Translation & Polish
- [ ] Text translation integration *(HTTP client for a local LibreTranslate instance exists in `translation/client.rs`; the `translate_text` command handler is still a stub)*
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
