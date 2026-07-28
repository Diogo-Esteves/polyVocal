# Architecture & Technology Decisions

This log records key decisions made during design and development, along with the reasoning and alternatives considered.

---

## Decision Log

| # | Date | Decision | Status |
|---|------|----------|--------|
| DEC-001 | 2026-07-28 | Tech stack: Tauri + Rust + whisper-rs | ✅ Accepted |

---

---

### [DEC-001] Tech Stack: Tauri + Rust + whisper-rs

**Date:** 2026-07-28  
**Status:** ✅ Accepted

**Context:**  
PolyVocal requires a cross-platform desktop app with low-latency local audio processing and ML inference. The team has strong Rust familiarity and prioritises performance, small binary size, and a clear path to mobile.

**Decision:**  
Use **Tauri** as the desktop application framework with **Rust** as the sole backend language. Transcription powered by **whisper-rs** (Rust bindings over `whisper.cpp`). Frontend UI via a web layer (framework TBD — see DEC-002).

**Alternatives considered:**
- Wails + Go — easier ramp-up, but weaker Whisper bindings and no clear mobile path
- Flutter + Dart — best mobile story, but adds Dart as a third language and FFI complexity
- Electron — rejected on performance and size grounds

**Consequences:**
- Rust is the language of record for all core logic
- `whisper-rs` / `whisper.cpp` is the transcription engine — no Python runtime
- Audio capture: `cpal` crate (cross-platform)
- Storage: `sqlx` with SQLite
- All heavy work stays in Rust; the frontend is purely presentational
- Mobile path: Tauri Mobile (alpha) or reuse Rust core as a library in a future mobile shell

---

## Template

```
### [DEC-XXX] Title

**Date:** YYYY-MM-DD
**Status:** Proposed | Accepted | Rejected | Superseded

**Context:**
What situation or problem triggered this decision?

**Decision:**
What was decided?

**Alternatives considered:**
- Option A
- Option B

**Consequences:**
What does this enable or constrain going forward?
```
