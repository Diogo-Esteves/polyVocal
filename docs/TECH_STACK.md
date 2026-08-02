# PolyVocal — Tech Stack Analysis

**Status:** ✅ Decided — Tauri + Rust + whisper-rs (see DEC-001 in DECISIONS.md)  
**Last updated:** 2026-08-02

This document evaluates the viable technology options for PolyVocal's desktop app and backend, optimised for:
- Performance (low latency transcription)
- Long-term maintainability
- Cross-platform reach (Linux / macOS / Windows → mobile later)
- Ability to add features without rewriting
- Team comfort: **Go or Rust backend preferred**

---

## The Architecture Has Two Parts

```
┌─────────────────────────────────────┐
│           Desktop UI Layer          │  ← What the user sees and clicks
├─────────────────────────────────────┤
│    Core Engine (audio + models)     │  ← Where the real work happens
└─────────────────────────────────────┘
```

These can be the **same language** or **different** (engine in Rust/Go, UI in web tech). Most viable options mix them intentionally.

---

## Option A — Tauri + Rust

**Stack:** Rust (core engine + app backend) + Web frontend (React / Svelte / Vue)

### How it works
Tauri is a framework that wraps a web frontend (rendered via the OS's native WebView) with a Rust backend. The Rust layer handles all heavy work — audio capture, model inference, file I/O. The web layer handles the UI.

### Strengths
- ✅ **Rust is ideal for audio + ML** — zero-cost abstractions, no GC pauses, no memory issues
- ✅ `whisper-rs` (Rust bindings to `whisper.cpp`) exists and is mature
- ✅ Tiny binary: < 10 MB app shell (models are separate)
- ✅ True native performance — no Electron overhead
- ✅ Web UI means designers can work in familiar tools
- ✅ Strong cross-platform story (Linux/macOS/Windows/mobile via Tauri Mobile alpha)
- ✅ Active, fast-growing ecosystem
- ✅ Best long-term fit for a performance-sensitive tool

### Weaknesses
- ⚠️ Rust has a steep learning curve
- ⚠️ Tauri Mobile is still maturing (alpha/beta)
- ⚠️ WebView differences across platforms can cause minor UI inconsistencies
- ⚠️ Bridging Rust ↔ JS adds some boilerplate (though Tauri handles most of it)

### Whisper integration
```
whisper-rs (Rust) → whisper.cpp (C++) → runs locally, no Python
```
Fast, self-contained, no Python runtime dependency.

### Verdict
🟢 **Best overall fit for PolyVocal.** Aligns with Rust preference, delivers the best performance, smallest binary, and has a clear path to mobile. The learning investment in Rust pays off long-term.

---

## Option B — Wails + Go

**Stack:** Go (core engine + app backend) + Web frontend (React / Svelte / Vue)

### How it works
Wails is Go's equivalent of Tauri. Go backend handles logic, web frontend handles UI, wrapped in a native window.

### Strengths
- ✅ Go is much easier to learn than Rust
- ✅ Excellent concurrency model — great for streaming audio pipelines
- ✅ Fast compile times, good tooling
- ✅ Good cross-platform support
- ✅ Web UI (same as Tauri)

### Weaknesses
- ⚠️ Go's Whisper bindings (`go-whisper`) are less mature than `whisper-rs`
- ⚠️ Go has a garbage collector — introduces occasional micro-pauses (usually fine, but noticeable in ultra-low-latency audio)
- ⚠️ Wails is smaller community than Tauri
- ⚠️ No clear mobile story yet
- ⚠️ Binary size larger than Tauri (Go runtime bundled)

### Whisper integration
```
go-whisper (CGo bindings) → whisper.cpp (C++) → runs locally
```
Works, but CGo adds complexity and the bindings are less battle-tested.

### Verdict
🟡 **Solid fallback if Rust is a blocker.** Go is faster to get started in, but you trade away some performance and the mobile path is unclear. Good for a prototype or if the team is Go-first.

---

## Option C — Flutter (Desktop)

**Stack:** Dart (UI + app logic) + Rust or Go via FFI (audio engine)

### How it works
Flutter compiles to native code across platforms using Dart. The UI is drawn on its own canvas (not native widgets, not WebView). For heavy work like audio/ML, you call into a Rust or Go library via FFI or platform channels.

### Strengths
- ✅ Best cross-platform UI consistency (desktop + mobile from one codebase)
- ✅ Dart is approachable and productive
- ✅ Native-feeling performance on mobile especially
- ✅ Strong mobile story (iOS + Android)
- ✅ Good for reaching mobile without a rewrite

### Weaknesses
- ⚠️ Flutter Desktop is still maturing on Linux/Windows
- ⚠️ Dart is not a common systems language — adds another language to learn
- ⚠️ The audio/ML engine still needs to be written in Rust/Go and bridged
- ⚠️ FFI bridging is more complex than Tauri/Wails IPC
- ⚠️ Larger app bundle size
- ⚠️ Custom canvas rendering means less native OS look-and-feel

### Verdict
🟡 **Interesting if mobile parity is a near-term goal.** If you want iOS/Android within 6 months, Flutter gives you that from one codebase. But for a desktop-first MVP, it adds unnecessary complexity.

---

## Option D — Electron + Node.js / Python

**Stack:** Electron (Chromium + Node.js) + Python sidecar for Whisper

### Strengths
- ✅ Largest ecosystem, most hiring pool
- ✅ Fastest to prototype a UI
- ✅ Python Whisper integration is the most documented path

### Weaknesses
- ❌ Electron bundles an entire Chromium browser — 150-300 MB minimum
- ❌ High memory usage (often 300-600 MB just at idle)
- ❌ Python sidecar adds startup time, complexity, and packaging pain
- ❌ Contradicts performance goals
- ❌ Not aligned with Go/Rust preference

### Verdict
🔴 **Not recommended.** The performance and size penalties directly conflict with PolyVocal's goals. Useful only as a throwaway prototype.

---

## Comparison Table

| Criterion | Tauri/Rust | Wails/Go | Flutter/Dart | Electron |
|---|---|---|---|---|
| Performance | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐ |
| Binary size | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ | ⭐ |
| Whisper integration | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ |
| Learning curve | ⭐⭐ (hard) | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| Desktop maturity | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ |
| Mobile path | ⭐⭐⭐ (alpha) | ⭐ (unclear) | ⭐⭐⭐⭐⭐ | ⭐⭐ |
| Long-term fit | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐ |
| Community/ecosystem | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ |

---

## Transcription Engine Options

Regardless of app framework, the transcription engine is a separate concern:

| Engine | Language | Notes |
|---|---|---|
| **whisper.cpp** | C++ | The gold standard for local Whisper. Has Rust (`whisper-rs`) and Go bindings. |
| **whisper-rs** | Rust | Idiomatic Rust bindings over whisper.cpp. Best choice with Tauri. |
| **go-whisper** | Go | CGo bindings to whisper.cpp. Works but less mature. |
| **faster-whisper** | Python | CTranslate2-optimised Whisper. Fast but requires Python runtime. |
| **Vosk** | C/Python | Lighter models, good offline, less accurate than Whisper. |
| **Candle (Whisper)** | Rust | Pure-Rust ML framework by HuggingFace — no C++ dependency. Promising but newer. |

**Recommended for MVP:** `whisper-rs` (with Tauri) or `go-whisper` (with Wails).

---

## Recommended Path

```
MVP:    Tauri (Rust) + whisper-rs + React/Svelte frontend
Proto:  Wails (Go) + go-whisper  ← if speed of initial prototype matters more
Mobile: Tauri Mobile (when stable) or Flutter shell wrapping the same Rust engine
```

---

## Decision

| Date | Decision | Notes |
|---|---|---|
| 2026-07-28 | ✅ **Tauri + Rust + whisper-rs** | Recorded as DEC-001 in DECISIONS.md |

> Once decided, record in `DECISIONS.md` as DEC-001.
