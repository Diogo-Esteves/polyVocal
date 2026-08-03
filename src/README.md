# Frontend

> Decided: **Leptos + WASM**, zero hand-written JavaScript (see DEC-002 in `docs/DECISIONS.md`).

This directory will contain the Leptos frontend for the Tauri app shell, compiled to
WebAssembly and served inside Tauri's WebView.

## Stack

- **Leptos** — signals-based reactivity, similar to Solid.js
- **trunk** — build tool, replaces `npm run dev` / `npm run build` in `tauri.conf.json`
- **tauri-sys** — Rust bindings for Tauri IPC (`invoke`, `listen`) from WASM
- Live transcript segments arrive via Tauri push events (`transcript:segment`, per DEC-007),
  not polling — the frontend only listens and renders.

Initialise with:
```bash
cargo install trunk
cargo install cargo-leptos
cargo leptos new --git https://github.com/leptos-rs/start
```
