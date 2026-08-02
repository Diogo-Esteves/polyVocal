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
- [ ] Transcription via local Whisper model *(VAD segmentation + engine interface in place; whisper-rs inference itself not yet wired — needs a bundled model)*
- [ ] Language auto-detection
- [ ] Basic UI (single screen)
- [ ] Linux + macOS + Windows support

## Phase 2 — Translation & Polish
- [ ] Text translation integration
- [ ] Model switcher (local ↔ cloud)
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
