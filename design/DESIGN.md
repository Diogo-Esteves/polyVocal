# PolyVocal — Design Document

> UI/UX principles, visual language, and interaction guidelines.

---

## Brand Concept

**Name:** PolyVocal  
**Tagline (draft):** *Every voice. Every language.*  
**Logo concept:** A hairbrush — drawing on the universal, playful image of someone singing into a hairbrush. Expressive, human, uninhibited.

---

## Design Principles

1. **Simplicity first** — One action per screen. No clutter.
2. **Speed feels real** — Visual feedback must be immediate, even if processing takes time.
3. **Language-neutral UI** — Interface works regardless of the language being spoken or displayed.
4. **Accessible** — High contrast, readable fonts, keyboard navigability.

---

## Visual Identity (TBD)

- [ x ] Logo (hairbrush-inspired)
- [ x ] Primary colour palette
- [x] Typography — OS-native font stack (`system-ui, -apple-system, "Segoe UI",
      Roboto, "Noto Sans", "Helvetica Neue", Arial, sans-serif`, see
      `src/styles.css`). No webfont fetch (speed), and it defers glyph
      coverage to whatever the OS already ships — a single bundled font
      can't cover every script the transcript/translation panes might need
      to render, so relying on the platform's own fallback chain is the
      language-neutral choice. It also inherits the user's OS-level font
      size and contrast accessibility settings for free.
- [x] Icon set — [Lucide](https://lucide.dev) (ISC license), a handful of
      icons hand-inlined as SVG `#[component]`s in `src/src/main.rs`
      (`mic`, `square`, `languages`, `triangle-alert`, `sun`, `moon`,
      `sun-moon`) rather than pulled in as a package, since the single
      screen only needs a few. All are stroke-based, `currentColor` (so
      they follow the theme automatically), and `aria-hidden` — every icon
      is paired with a text label, so none of them carry meaning on their
      own (language-neutral, accessible).
- [x] Dark / light mode — CSS custom properties in `src/styles.css`.
      `prefers-color-scheme` is the default (required baseline, zero JS/WASM
      needed, reacts live to OS changes); a three-state toggle in the header
      (`Auto → Light → Dark → Auto`, `src/src/main.rs`) can override it
      explicitly via a `data-theme` attribute on `<html>`, persisted to
      `localStorage` through `web-sys` (kept in Rust/WASM, per DEC-002 —
      no hand-written JS). Colors were picked for contrast: body text is
      ≥7:1 against its background in both themes, and interactive elements
      (buttons, focus rings) are ≥4.5:1.

---

## Key Screens (planned)

- **Home / Record** — Large, obvious record button. Current language shown. Recent transcripts listed.
- **Transcript view** — Flowing text output, translation toggle, copy/export actions.
- **Settings** — Model selection (local / cloud), language preferences, sync settings.

---

## Interaction Patterns

- **Push-to-talk** or **toggle record** — user chooses in settings
- **Live waveform** — visual cue that audio is being captured
- **Inline translation** — toggle between original and translated text without leaving the screen

---

## Ideas — under consideration

Feature concepts pulled from a competitor screenshot the user shared
(2026-08-05) — functional ideas only, not the visual design. None of these
are scoped or on `docs/ROADMAP.md` yet; they need to be clarified into
concrete tasks before implementation.

- **Per-speaker audio** — capture and meter "You" and "Others" as separate
  streams, each with its own level meter and controls, instead of one
  undifferentiated mic input. Would be a real architecture change (current
  pipeline assumes a single speaker/source).
- **Selective translation** — translate only the "Others" stream live, leave
  the user's own speech untranslated, as a per-source toggle rather than
  today's one-shot "translate the whole transcript on demand" action.
- **Live language override** — change the active language mid-session
  (inline dropdown next to record/stop), instead of only relying on
  auto-detect or setting language before/after recording.
- **Pause, distinct from stop** — current UI only has record/stop; no way
  to pause and resume within one session.
- **Audio-processing toggle with inline contextual help** — e.g. an "echo
  reduction" toggle with its tradeoff (latency, headphones-vs-speakers)
  shown directly under the control, not buried in a separate settings
  screen.
- **Multiple structured views of a session** — raw Transcription / Session
  (metadata) / AI-generated Summary as separate tabs on the same session,
  rather than one flat transcript view.
- **System-wide floating/overlay control bar** — a compact always-on-top
  control surface independent of the main window, for controlling
  recording from anywhere. Complements the already-roadmapped "global
  hotkey" item (see root `README.md`).
- **Quick "mark this moment" action** — a keyboard-shortcut-triggered
  marker/bookmark dropped into the transcript at the current timestamp
  while recording.
- **Assistant/persona mode selector** — implies a conversational AI layer
  on top of transcription (a "general assistant" you can ask things), which
  is a materially different and bigger feature direction than transcription
  + translation — needs its own discussion before it's anything more than
  an idea.

---

*Wireframes to be added here.*
