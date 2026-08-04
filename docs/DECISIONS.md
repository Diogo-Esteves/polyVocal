# Architecture & Technology Decisions

This log records every key decision made during design and development, along with reasoning and alternatives considered.

---

## Decision Log

| # | Date | Decision | Status |
|---|------|----------|--------|
| DEC-001 | 2026-07-28 | Tech stack: Tauri + Rust + whisper-rs | ✅ Accepted |
| DEC-002 | 2026-07-28 | Frontend: Leptos + WASM (full Rust, no JS) | ✅ Accepted |
| DEC-003 | 2026-07-28 | Core transcription loop: VAD-gated segments | ✅ Accepted |
| DEC-004 | 2026-07-28 | VAD model: Silero via ONNX (`ort` crate) | ✅ Accepted |
| DEC-005 | 2026-07-28 | Audio pipeline: always resample in Rust (`rubato`) | ✅ Accepted |
| DEC-006 | 2026-07-28 | Threading model: mixed OS thread + Tokio | ✅ Accepted |
| DEC-007 | 2026-07-28 | Live results to frontend: Tauri push events | ✅ Accepted |
| DEC-008 | 2026-07-28 | App state: per-domain `Arc<Mutex<T>>` | ✅ Accepted |
| DEC-009 | 2026-07-28 | Session persistence: incremental + finalise | ✅ Accepted |
| DEC-010 | 2026-07-28 | Translation engine: OPUS-MT via `candle` | ✅ Accepted |
| DEC-011 | 2026-07-28 | Model distribution: bundle tiny + bring-your-own | ✅ Accepted |
| DEC-012 | 2026-07-28 | Keyboard interaction: hold/toggle + opt-in global hotkey | ✅ Accepted |
| DEC-013 | 2026-07-28 | Settings persistence: TOML config file | ✅ Accepted |
| DEC-014 | 2026-07-28 | Error handling: tiered by severity | ✅ Accepted |
| DEC-015 | 2026-07-28 | Sync foundation: UUIDs + device ID + wire format | ✅ Accepted |
| DEC-016 | 2026-07-28 | Auto-update: Tauri updater plugin | ✅ Accepted |
| DEC-017 | 2026-07-28 | Freemium boundary: open models + Pro = sync + translation | ✅ Accepted |
| DEC-018 | 2026-07-28 | Logging: opt-in crash reporting, no transcript data | ✅ Accepted |
| DEC-019 | 2026-07-28 | Testing: unit + integration | ✅ Accepted |

---

### [DEC-001] Tech Stack: Tauri + Rust + whisper-rs

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
PolyVocal requires a cross-platform desktop app with low-latency local audio processing and ML inference. The team has strong Rust familiarity and prioritises performance, small binary size, and a clear path to mobile.

**Decision:**
Use **Tauri** as the desktop application framework with **Rust** as the sole backend language. Transcription powered by **whisper-rs** (Rust bindings over `whisper.cpp`).

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

---

### [DEC-002] Frontend: Leptos + WASM

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
Tauri needs a frontend. The team wants zero JavaScript — full Rust across the entire stack.

**Decision:**
Use **Leptos** compiled to **WebAssembly** running inside Tauri's WebView. Build tooling via `trunk`. Tauri IPC via `tauri-sys` crate.

**Alternatives considered:**
- Svelte/Solid.js/React — all require JavaScript
- Dioxus Desktop — no WASM, native renderer, but Tauri's packaging/update ecosystem is more mature
- Yew — older Rust/WASM framework, slower evolution than Leptos

**Consequences:**
- Zero JavaScript written by hand
- Two Rust compilation targets: `x86_64-*` (backend) and `wasm32-unknown-unknown` (frontend)
- `trunk` replaces `npm run dev` as the frontend build tool
- Signals-based reactivity model (similar to Solid.js)
- All types shared between frontend and backend are `serde`-serialisable

---

### [DEC-003] Core Transcription Loop: VAD-Gated Segments

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
Whisper inference runs on discrete audio buffers, not a continuous stream. The question is what defines a buffer boundary.

**Decision:**
Use **Voice Activity Detection (VAD)** to gate utterances. Audio is fed to Whisper only when VAD detects a complete speech segment (speech followed by silence). Text appears after each utterance, not mid-word.

**Alternatives considered:**
- Fixed-size time chunks (e.g. 5–10s) — simple but causes word-boundary errors and visible lag

**Consequences:**
- Requires a VAD stage in the pipeline before Whisper inference
- Transcript latency is per-utterance, not per-chunk — feels more natural
- Accuracy significantly better than chunk-based approach

---

### [DEC-004] VAD Model: Silero via ONNX

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
VAD requires a model. Options range from classical DSP algorithms to modern ML models.

**Decision:**
Use **Silero VAD** running via **ONNX Runtime** (`ort` Rust crate). No Python runtime. Adds ~15 MB to the app.

**Alternatives considered:**
- WebRTC VAD — pure C, ~50KB, but less accurate on accented/multilingual speech
- Whisper built-in VAD — coarse, not reliable enough for production

**Consequences:**
- `ort` crate added as a dependency
- Silero ONNX model bundled with the app (~1.8 MB)
- Inference runs in ~1ms per audio frame — negligible overhead

---

### [DEC-005] Audio Pipeline: Always Resample in Rust

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
Whisper requires 16 kHz mono f32 PCM. Microphones deliver varying formats (44.1/48 kHz, stereo, i16). The OS audio driver cannot be trusted to honour a requested format across platforms.

**Decision:**
Always resample and convert in Rust using **`rubato`**, regardless of what the OS delivers. Never rely on the driver to provide the correct format.

**Alternatives considered:**
- Request 16 kHz from `cpal` directly — unreliable across Linux/macOS/Windows drivers
- Resample only if needed (hybrid) — adds conditional logic, still needs the resampler

**Consequences:**
- `rubato` crate added as a dependency
- Pipeline is deterministic across all platforms
- Minimal external dependencies principle upheld

---

### [DEC-006] Threading Model: Mixed OS Thread + Tokio

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
Audio capture is latency-sensitive and must never be blocked by the async scheduler. Downstream processing (VAD, Whisper inference) is CPU-bound and can run on a thread pool.

**Decision:**
- **Dedicated OS thread** for audio capture (`cpal` callback) — low jitter, never blocked
- **Tokio `spawn_blocking`** for VAD and Whisper inference — CPU-bound, off the async executor
- **Tokio async** for orchestration, IPC, storage, and event emission
- **`std::sync::mpsc` channel** bridges the OS thread to the Tokio runtime

**Alternatives considered:**
- Pure Tokio async — audio callbacks can't be async, GC-style scheduler pauses are unacceptable
- Pure OS threads — loses the ergonomics of async for network, IPC, and storage

**Consequences:**
- Audio pipeline has real-time guarantees
- Whisper inference doesn't block the event loop
- Clear ownership boundaries between threads

---

### [DEC-007] Live Results to Frontend: Tauri Push Events

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
Transcribed utterances need to reach the Leptos UI as they are produced, in real time.

**Decision:**
Use **Tauri's event system** (`app_handle.emit()` on the Rust side, `listen()` via `tauri-sys` on the Leptos/WASM side). Each completed utterance emits a `transcript:segment` event.

**Alternatives considered:**
- Frontend polling via Tauri commands — wasteful, adds latency, fights the real-time model
- Long-lived streamed commands — more complex to implement with WASM

**Consequences:**
- Push-based, zero polling, low latency
- Frontend is purely reactive — it listens and renders, never asks
- Event payload: `{ session_id, text, language, start_ms, end_ms }`

---

### [DEC-008] App State: Per-Domain `Arc<Mutex<T>>`

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
Multiple threads and Tauri command handlers need to share mutable state. A single global state struct becomes a bottleneck and a god object.

**Decision:**
Split state by domain, each registered separately via Tauri's `manage()`:
- `Arc<Mutex<AudioState>>` — is recording? current device?
- `Arc<Mutex<SessionState>>` — current session ID, partial transcript
- `Arc<Mutex<ModelState>>` — active Whisper model, active OPUS-MT pair

**Alternatives considered:**
- Single `Arc<Mutex<AppState>>` — simple but becomes a god object
- Actor model (Tokio channels) — cleanest but overkill for this state surface

**Consequences:**
- Fine-grained locking — audio state and model state never contend
- Each module owns only what it needs
- Straightforward to extend as new domains are added

---

### [DEC-009] Session Persistence: Incremental + Finalise

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
A user could record for 20+ minutes. A crash mid-session must not lose all data.

**Decision:**
- **Write each segment** to SQLite as it arrives from Whisper
- **Finalise on stop** — `UPDATE` the session record with total duration, final detected language, and `status = complete`

**Alternatives considered:**
- Write on stop only — loses all data on crash
- Write incrementally only — no clean final record for querying/syncing

**Consequences:**
- Crash-safe — at worst the last utterance is lost
- Each segment has its own UUID — sync-ready from day one
- SQLite handles frequent small writes efficiently

---

### [DEC-010] Translation Engine: OPUS-MT via `candle`

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
Translation must be local, offline-capable, and pure Rust — consistent with the minimal-dependencies principle.

**Decision:**
Use **OPUS-MT** translation models run via **HuggingFace `candle`** (pure Rust ML framework). No Python, no sidecar process. Models are small (~300 KB per language pair).

**Alternatives considered:**
- LibreTranslate sidecar — Python runtime, adds ~300 MB to installer
- User-managed LibreTranslate server — terrible UX
- Argos Translate Rust bindings — immature, narrower language coverage
- Cloud translation APIs — contradicts local-first principle for MVP

**Consequences:**
- `candle` added as a dependency alongside `whisper-rs`
- Both ML inference engines use the same underlying framework
- MVP language pairs: `en↔pt`, `en↔es`, `pt↔es`
- Translation is a post-processing step triggered by user action

**Implementation update (2026-08-04):** replaced the interim LibreTranslate
HTTP client (issue #20) with the local `candle` engine described above
(issue #35). A few details only became clear during implementation and
amend the consequences above:

- **Crate versions:** `candle-core`/`candle-nn`/`candle-transformers` 0.11.
  No accelerated backend (`mkl`/`accelerate`/`cuda`) is enabled — candle's
  default CPU backend (the `gemm` crate) dispatches on CPU features at
  *runtime*, unlike `whisper-rs-sys`'s `GGML_NATIVE` (see DEC-001's CI
  fix), which bakes `-march=native` in at *build* time. So there's no
  equivalent "built on one CI runner, SIGILLs on another" risk to guard
  against here, and no CI change was needed.
- **No single Helsinki-NLP model covers every MVP pair.** `en↔es` and
  `en→pt` have direct OPUS-MT models; `pt→en` has no dedicated model, so it
  goes through `Helsinki-NLP/opus-mt-ROMANCE-en` (many Romance source
  languages, including pt, to a fixed English target); `pt↔es` has no
  model in either direction, so it's translated in two hops, pivoting
  through English (`pt→en→es`, `es→en→pt`) using the models above.
- **Tokenization needs more than `candle` + a `tokenizers` JSON.** OPUS-MT
  repos ship a raw SentencePiece `.model` file and a plain `vocab.json`,
  not a HF fast-tokenizer JSON. The `rust_tokenizers` crate (pure Rust,
  reimplements SentencePiece's segmentation, Marian's `>>lang<<` prefix
  handling, and NFKC normalisation) is used instead of FFI-binding the
  reference SentencePiece C++ library: an FFI binding statically vendors
  its own copy of Google's `protobuf-lite`, which collides at link time
  with the copy already bundled inside `ort`'s downloaded static
  `onnxruntime` (DEC-004) — both define the same C++ symbols, and the
  linker refuses to merge two conflicting definitions.
- **Weights load from `model.safetensors`, not `pytorch_model.bin`.**
  Several of these repos' `main` branch only publishes the legacy pre-1.6
  PyTorch pickle format (no zip container), which candle's pickle reader
  doesn't support. Where `main` lacks a `safetensors` conversion, weights
  are pulled from the HuggingFace auto-conversion bot's (unmerged, but
  functional) PR ref instead — the same approach candle's own `marian-mt`
  example uses for several of these exact repos.
- **Real model sizes are ~300–450 MB per underlying model file** (four
  files cover all three MVP pairs bidirectionally, via the pivot above),
  not the ~300 KB originally estimated above — that figure appears to have
  been an order-of-magnitude-scale placeholder rather than a measurement.
  This also corrects DEC-011's "~1 MB" bundled-pair figure; see its own
  consequences.
- Models download on demand via the existing `ModelManager` /
  `ModelDownloader` machinery (same pattern as Whisper/Silero), not
  bundled in the installer.

---

### [DEC-011] Model Distribution: Bundle Tiny + Bring-Your-Own

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
Whisper and OPUS-MT models are too large to bundle entirely, but shipping an empty app creates a poor first-run experience.

**Decision:**
- **Bundle in installer:** `ggml-tiny.bin` (~75 MB) + `en↔pt` OPUS-MT pair (~1 MB)
- **App works immediately** after install — no setup wizard
- **Settings panel** allows downloading `base`, `small`, `medium` Whisper models
- **Bring-your-own models** — users can point PolyVocal at any compatible GGUF/GGML model downloaded from HuggingFace or elsewhere

**Alternatives considered:**
- Download on first launch (wizard) — blocks first use
- Download on demand — app launches but feels broken
- Separate language pack installers — fragmented UX

**Consequences:**
- Installer size ~80 MB
- Model manager UI needed in Settings
- Open model system — users are never locked into PolyVocal's hosted models

**Amendment (2026-08-04):** the "~1 MB" `en↔pt` OPUS-MT figure above was
wrong — see DEC-010's implementation update. Real OPUS-MT model files run
~300–450 MB each, too large to bundle in the installer alongside
`ggml-tiny.bin`. Translation models are downloaded on first use instead
(same on-demand pattern as the non-bundled Whisper sizes); only the
bundled `tiny` Whisper model still ships in the installer.

---

### [DEC-012] Keyboard Interaction: Hold/Toggle + Opt-In Global Hotkey

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
Professional users need to trigger recording without switching windows. But global hotkeys require OS permissions not all users want to grant.

**Decision:**
- **Both hold-to-talk and toggle modes** — user selects in Settings
- **In-app shortcut** always works (default)
- **Global hotkey** — opt-in in Settings, triggers OS permission request on macOS/Linux
- Via `tauri-plugin-global-shortcut`

**Alternatives considered:**
- In-app only — limits productivity for the core professional use case
- Global only — forces a permission grant on all users

**Consequences:**
- Default install: zero extra permissions
- Power users unlock global hotkey intentionally
- Principle established: give users maximum control over configuration

---

### [DEC-013] Settings Persistence: TOML Config File

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
User preferences (active model, microphone, interaction mode, hotkey binding, translation language) must persist across restarts.

**Decision:**
Store all settings in a **`config.toml`** file in the platform app data directory. Serialised/deserialised with `serde` + `toml` crate. Loaded at startup, written on every change.

**Alternatives considered:**
- SQLite (same DB as transcripts) — mixes concerns
- Tauri `plugin-store` — adds dependency, JSON is less human-readable
- Platform-native (registry / plist / XDG) — three implementations to maintain

**Consequences:**
- Human-readable and hand-editable by power users
- Zero extra dependencies beyond `toml` crate
- Migration strategy deferred — format can evolve with `serde` defaults

---

### [DEC-014] Error Handling: Tiered by Severity

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
Different failures demand different responses. A blanket strategy is wrong for at least one class of error.

**Decision:**
Three tiers:
1. **Silent recovery** — transient errors (dropped audio chunk, single VAD miss). Log internally, retry if possible, never interrupt the user.
2. **Toast notification** — recoverable failures the user should know about (model download failed, translation unavailable). Non-blocking, dismissible.
3. **Blocking dialog** — critical startup failures only (no microphone found, no model present, database corrupt). Requires user action.

**Alternatives considered:**
- Silent only — user has no visibility into meaningful failures
- Always toast — startup failures need more than a dismissible toast
- Always dialog — catastrophically disruptive for transient errors

**Consequences:**
- Each error site in the codebase must be classified at one of three tiers
- Toast component needed in Leptos UI
- Tauri `plugin-dialog` used for blocking dialogs

---

### [DEC-015] Sync Foundation: UUIDs + Device ID + Wire Format

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
Sync is post-MVP but the data model must be designed now to avoid a painful schema migration later.

**Decision:**
Every session and segment gets:
- `id: UUID v4` — globally unique
- `device_id: String` — which device created it
- `created_at: RFC3339 timestamp`
- `synced: bool` — has this been pushed to other devices?

Additionally, the **sync wire format** is documented now (see below) even though nothing sends it yet.

**Sync wire format (v1):**
```json
{
  "device_id": "uuid",
  "schema_version": 1,
  "sessions": [
    {
      "id": "uuid",
      "device_id": "uuid",
      "created_at": "2026-07-28T10:00:00Z",
      "duration_ms": 12400,
      "language": "en",
      "transcript": "...",
      "translation": "...",
      "target_lang": "pt",
      "segments": [
        { "id": "uuid", "start_ms": 0, "end_ms": 3200, "text": "..." }
      ]
    }
  ]
}
```

**Conflict resolution:** last-write-wins (by `created_at`). CRDT upgrade path available later.

**Alternatives considered:**
- UUIDs only (minimal) — requires schema migration when sync is built
- Full CRDT model — overkill for append-only, device-owned transcripts

**Consequences:**
- Schema is sync-ready from day one
- No schema migration needed when sync is activated
- Wire format contract is established before the first sync line of code

---

### [DEC-016] Auto-Update: Tauri Updater Plugin

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
Users need to receive new versions. PolyVocal should notify them without forcing an update.

**Decision:**
Use **`tauri-plugin-updater`**. On launch, check a version manifest hosted on **GitHub Releases**. If a new version is available, show a non-blocking notification. User chooses when to update.

**Alternatives considered:**
- Manual downloads only — poor UX for professional users
- Platform package managers (Homebrew, Flatpak, winget) — add later when distribution matures
- Silent auto-install — removes user control, unacceptable

**Consequences:**
- Zero server infrastructure — GitHub Releases hosts the manifest
- User is always in control of when they update
- Package manager support added in Phase 3+ as distribution grows

---

### [DEC-017] Freemium Boundary: Open Models + Pro Services

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
PolyVocal is freemium. The line between free and paid must be concrete and architecturally reflected.

**Decision:**
- **Free tier:** `tiny` model, EN/PT/ES transcription, core transcription only, no sync
- **Open model system:** any user (free or Pro) can download and use any compatible model from HuggingFace — the transcription engine is never locked
- **Pro tier:** sync across devices, translation (OPUS-MT), all future value-add features
- **Enterprise:** volume, self-hosted sync, API access (future)

**Alternatives considered:**
- Model size gating only — arbitrary, frustrating for power users
- Language gating — undermines the multilingual mission
- Usage gating — requires tracking, privacy concern

**Consequences:**
- Licence check needed in Rust backend for Pro features (sync, translation)
- Core transcription engine is always open — strong trust signal
- Pro value is in services, not in crippling the free tier

---

### [DEC-018] Logging: Opt-In Crash Reporting

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
PolyVocal handles sensitive audio and transcripts. Logging must be designed with privacy as a hard constraint.

**Decision:**
- **Local logs always:** `tracing` writes to a rotating log file in the app data directory
- **Opt-in crash reporting:** on first launch, user is asked to opt in to anonymous crash reports
- **Hard guarantee:** transcript content, audio data, and file paths are **never** included in crash payloads — stack traces and system info only
- **No usage analytics, ever**

**Alternatives considered:**
- No telemetry at all (D) — considered seriously; B chosen because crash signal is valuable for quality without compromising privacy
- Opt-in usage analytics — rejected, higher privacy bar than warranted

**Consequences:**
- Privacy policy must explicitly state the guarantee
- Crash reporter must be implemented with content scrubbing
- `tracing` + `tracing-subscriber` already in the dependency tree

---

### [DEC-019] Testing Strategy: Unit + Integration

**Date:** 2026-07-28
**Status:** ✅ Accepted

**Context:**
A Rust + Tauri + WASM stack has distinct layers. Testing investment must be proportional to value.

**Decision:**
- **Unit tests** for all pure Rust logic: resampler, VAD integration, session repository, config serialisation, model registry
- **Integration tests** for the audio pipeline: feed known audio fixtures in multiple languages, assert transcript output matches expectation — catches regressions in the Whisper/VAD pipeline
- **No E2E tests** at this stage — the app is used daily by the developer; bugs are caught in use

**Alternatives considered:**
- Unit only — misses pipeline-level regressions
- Unit + integration + E2E — high effort, brittle, poor ROI at this stage
- Unit + integration + UI snapshots — reasonable future addition when UI stabilises

**Consequences:**
- A `fixtures/` directory of known audio samples (multiple languages) maintained in the repo
- Integration test suite runs `cargo test` — no special tooling needed
- E2E added in Phase 3 when UI stabilises and a second developer joins

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
