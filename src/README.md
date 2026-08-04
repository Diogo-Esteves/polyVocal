# Frontend

**Leptos + WASM**, zero hand-written JavaScript (see DEC-002 in `docs/DECISIONS.md`).

This directory contains the Leptos frontend for the Tauri app shell, compiled to
WebAssembly and served inside Tauri's WebView. It's a single-screen MVP (Phase 1,
`docs/ROADMAP.md`) — no routing, no settings screen.

## Stack

- **Leptos** (`csr` feature) — signals-based reactivity, similar to Solid.js
- **trunk** — build tool, invoked by `tauri.conf.json`'s `beforeDevCommand`/`beforeBuildCommand`
  in place of `npm run dev` / `npm run build`
- **tauri-sys** — Rust bindings for Tauri IPC (`invoke_result`, `listen`) from WASM
- Live transcript segments arrive via the `transcript:segment` Tauri push event (DEC-007),
  not polling — the frontend only listens and renders.

## Layout

- `src/main.rs` — the whole single-screen UI: record/stop control, live transcript
  (via `transcript:segment`), detected source language, and a translate action
  (target language + `translate_text`); also the inlined icon set and the
  dark/light theme toggle (see `design/DESIGN.md`)
- `styles.css` — typography, color tokens (light/dark), and component styles;
  linked from `index.html` via trunk's `data-trunk rel="css"`
- `index.html` — trunk's entry point
- `Trunk.toml` — dev server port (must match `tauri.conf.json`'s `devUrl`) and dist dir

## Local development

```bash
cargo install trunk
rustup target add wasm32-unknown-unknown
trunk serve   # or: cargo tauri dev, from src-tauri/, which drives this automatically
```
