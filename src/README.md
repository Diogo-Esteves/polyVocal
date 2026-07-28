# Frontend

> Framework decision pending — **React** or **Svelte** (see DEC-002 in DECISIONS.md).

This directory will contain the web frontend for the Tauri app shell.

## Candidates

| Framework | Bundle size | DX | Notes |
|---|---|---|---|
| **Svelte** | Tiny — no runtime | Excellent | Best fit for a lean, fast Tauri app |
| **React** | Small (with Vite) | Familiar | Larger ecosystem, more hiring pool |

Once decided, initialise with:
```bash
# Svelte
npm create vite@latest . -- --template svelte-ts

# React
npm create vite@latest . -- --template react-ts
```
