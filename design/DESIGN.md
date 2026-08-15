# PolyVocal — Design Document

> UI/UX principles, visual language, and interaction guidelines.

---

## How to read this document

This document describes the **target design**. Not all of it is built.
Every section is tagged so the doc doesn't drift into describing features
that don't exist:

- **`[built]`** — implemented and shipping today.
- **`[spec]`** — designed and agreed, not yet implemented. Tracked by a
  GitHub issue.

When you implement a `[spec]` section, flip its tag and update the text to
match what actually shipped — not what was planned.

---

## Brand Concept

**Name:** PolyVocal
**Tagline (draft):** *Every voice. Every language.*
**Logo concept:** A hairbrush — drawing on the universal, playful image of
someone singing into a hairbrush. Expressive, human, uninhibited.

**The insight that drives the whole UI:** in that image, the hairbrush *is
the microphone*. So the brush is not decoration sitting in a corner — it is
the record button, and the coloured strands flying off it are the live
audio. One element carries the brand, the primary action, and the "audio is
being captured" feedback at once.

Source art: `assets/logo/logo.png` (1024px raster, used in `README.md`).

---

## Design Principles

1. **Simplicity first** — One action per screen. No clutter.
2. **Speed feels real** — Visual feedback must be immediate, even if
   processing takes time.
3. **Language-neutral UI** — Interface works regardless of the language
   being spoken or displayed.
4. **Accessible** — High contrast, readable fonts, keyboard navigability.
5. **Languages, not files** — Users pick "Portuguese → English", never
   `opus-mt-romance-en` or a `.bin` filename. Model management is an
   implementation detail that leaks only where it must (download size,
   progress).

---

## Design Tokens `[spec]`

The palette is derived from the logo: cream ground, wood handle, and the
four strand colours. It replaces the previous zinc/blue neutral palette,
which read as a developer tool rather than something a person would choose
to talk into.

**Contrast policy.** Body text is ≥7:1 against its background (WCAG AAA).
Secondary text, interactive fills, and any graphic that carries meaning are
≥4.5:1 / ≥3:1 as applicable. Warm mid-tones are decorative only — never
text on background. Every ratio below is measured, not estimated.

### Light

| Token | Value | Role | Contrast |
|---|---|---|---|
| `--color-bg` | `#F7F2E7` | app ground (cream) | — |
| `--color-surface` | `#FFFFFF` | cards, sheets, inputs | — |
| `--color-surface-sunken` | `#EFE7D7` | transcript well, inset areas | — |
| `--color-text` | `#2A2420` | body text | 13.71:1 on bg |
| `--color-text-muted` | `#6B5F52` | meta, labels, timestamps | 5.56:1 on bg |
| `--color-hairline` | `#E4DAC6` | decorative separators only | 1.24:1 (intentional) |
| `--color-border` | `#8A7B66` | control boundaries | 3.68:1 on bg |
| `--color-primary` | `#8A5A32` | wood — buttons, active state | 5.24:1 on bg |
| `--color-primary-contrast` | `#FFF9EF` | text on primary fill | 5.59:1 on primary |
| `--color-recording` | `#C25A3E` | recording ring | 3.90:1 on bg |
| `--color-danger` | `#A3231B` | destructive text/icon | 6.69:1 on bg |
| `--color-danger-bg` | `#F7DDD6` | destructive surface | 5.78:1 for danger text |
| `--focus-ring` | `#8A5A32` | focus outline | 5.24:1 on bg |

Strands (light) — darkened so they stay ≥3:1 while the meter carries meaning:

| Token | Value | Contrast |
|---|---|---|
| `--strand-1` teal | `#3F7D79` | 4.25:1 |
| `--strand-2` coral | `#C25A3E` | 3.90:1 |
| `--strand-3` amber | `#A87C22` | 3.38:1 |
| `--strand-4` sage | `#5F7F52` | 4.05:1 |

### Dark

| Token | Value | Role | Contrast |
|---|---|---|---|
| `--color-bg` | `#16130F` | app ground (warm near-black) | — |
| `--color-surface` | `#211D18` | cards, sheets, inputs | — |
| `--color-surface-sunken` | `#0F0D0A` | transcript well | — |
| `--color-text` | `#F4EEE2` | body text | 16.02:1 on bg |
| `--color-text-muted` | `#B5A895` | meta, labels | 7.94:1 on bg |
| `--color-hairline` | `#302A22` | decorative separators only | 1.30:1 (intentional) |
| `--color-border` | `#7A6E5D` | control boundaries | 3.72:1 on bg |
| `--color-primary` | `#D9A06B` | wood, lightened for dark | 8.10:1 on bg |
| `--color-primary-contrast` | `#1A130C` | text on primary fill | 8.05:1 on primary |
| `--color-recording` | `#E8836B` | recording ring | 6.96:1 on bg |
| `--color-danger` | `#F2857B` | destructive text/icon | 7.42:1 on bg |
| `--color-danger-bg` | `#3A1A16` | destructive surface | 6.28:1 for danger text |
| `--focus-ring` | `#D9A06B` | focus outline | 8.10:1 on bg |

Strands (dark) — the logo's original brights, which already clear 3:1 on
near-black: teal `#7FB3B0` (7.92:1), coral `#E8836B` (6.96:1), amber
`#EFC05F` (10.91:1), sage `#9BAF8E` (7.86:1).

### Typography `[built]`

OS-native stack — no webfont fetch (speed), and glyph coverage defers to
whatever the OS ships. A single bundled font can't cover every script the
transcript and translation panes might need, so the platform's own fallback
chain is the language-neutral choice. It also inherits the user's OS-level
font size and contrast settings for free.

```
system-ui, -apple-system, "Segoe UI", Roboto, "Noto Sans",
"Helvetica Neue", Arial, sans-serif
```

Scale `[spec]` — transcript text is deliberately the largest thing on the
screen, because it is the thing you are there to read:

| Step | Size | Use |
|---|---|---|
| `--text-xs` | 0.75rem | timestamps, chips |
| `--text-sm` | 0.875rem | meta, secondary labels |
| `--text-base` | 1rem | UI text, buttons |
| `--text-lg` | 1.125rem | translated line |
| `--text-xl` | 1.375rem | transcript line |

Never set a `px` font size — users who scale their OS text must be able to
scale the app.

### Spacing, radius, elevation `[spec]`

Spacing is a 4px scale: `--space-1` 0.25rem through `--space-8` 2rem.
Radii: `--radius-sm` 8px (chips, inputs), `--radius-md` 14px (cards,
sheets), `--radius-full` 9999px (record button, language pills). The
previous 6px radii read as utilitarian; softer corners suit the brand.

One elevation only, for sheets and the record button:
`0 2px 8px rgb(42 36 32 / 0.10)` light, `0 2px 8px rgb(0 0 0 / 0.4)` dark.
Flat everywhere else — elevation means "this floats above the app", and
nothing else does.

### Motion `[spec]`

| Token | Value | Use |
|---|---|---|
| `--motion-fast` | 120ms | button press, hover |
| `--motion-base` | 200ms | sheet open/close, toggles |
| `--motion-slow` | 1400ms | recording pulse cycle |

All of it sits behind `@media (prefers-reduced-motion: reduce)`, which
must disable the strand animation, the recording pulse, and sheet
transitions — not merely shorten them.

---

## The PolyVocal Mark `[spec]`

The raster logo can't theme, can't animate per-strand, and is soft at large
sizes. The mark is rebuilt as inline SVG — an interpretation of the source
art, not a trace of it.

**Anatomy**, on a `0 0 32 32` grid, drawn upright and rotated as a group so
the tilt is a single number to tune:

- **Paddle** — rounded oblong, centre `(13, 11)`, 12×15, `rx 6`, wood fill.
- **Bristle pad** — inset rounded rect, 8.5×11.5, `rx 4.25`, one step
  darker than the wood.
- **Bristles** — 6 short round-capped ticks on the pad. Omitted below 24px.
- **Handle** — 4.5×10 rounded stem from the paddle base, ending in a bulb
  (`r 3`) — the source art's silhouette depends on that bulb.
- **Strands** — 4 quadratic curves springing from the paddle's right edge,
  `stroke-width 2`, round caps, no fill, one per strand token.

```svg
<svg viewBox="0 0 32 32" fill="none" aria-hidden="true">
  <g transform="rotate(-15 16 16)">
    <rect x="9.75" y="16" width="4.5" height="10" rx="2.25" fill="var(--wood)"/>
    <circle cx="12" cy="26" r="3" fill="var(--wood)"/>
    <rect x="7" y="3.5" width="12" height="15" rx="6" fill="var(--wood)"/>
    <rect x="8.75" y="5.25" width="8.5" height="11.5" rx="4.25" fill="var(--wood-dark)"/>
  </g>
  <g class="pv-strands" stroke-width="2" stroke-linecap="round">
    <path d="M17 15 Q25 16 28 12" stroke="var(--strand-4)"/>
    <path d="M17 13 Q24 12 26  7" stroke="var(--strand-1)"/>
    <path d="M18 10 Q25 10 29  6" stroke="var(--strand-2)"/>
    <path d="M18  8 Q24  6 28  4" stroke="var(--strand-3)"/>
  </g>
</svg>
```

**Size variants.** 512px — app icon, full detail. 96/88px — record button,
full detail. 24px — header lockup, drop the bristles. 20px and below —
paddle, handle, and two strands only; four strands turn to mush.

**The app icon must be regenerated from this mark.** `src-tauri/icons/` is
still the default Tauri placeholder — `128x128.png` is a plain blue square.
The shipped app currently has no brand identity at all.

---

## The Record Button `[spec]`

The single most important control in the app, and today its weakest: a
~38px chip that is geometrically identical to Export and Delete, differing
only in colour. It becomes a circular target with the mark at its centre.

**Anatomy** — 88px diameter on mobile, 96px on desktop (both far above the
44px touch minimum), `--color-surface` fill, 2px ring, one elevation step,
mark at 56% of the diameter, bristle-head up so it reads as a mic capsule.

| State | Ring | Mark | Strands | Label below |
|---|---|---|---|---|
| Idle | `--color-primary` | full colour | static, at rest | "Tap to record" |
| Recording | `--color-recording`, pulsing | full colour | animated, amplitude-driven | "0:05 · Tap to stop" |
| Processing | `--color-primary`, static | full colour | slow shimmer | "Transcribing…" |
| Disabled | `--color-border` | 50% opacity | static | reason for being disabled |

**Amplitude.** The strand group scales from a transform-origin at the
paddle, driven by a `--pv-amp` custom property in `0..1`, so the strands
lengthen and spread with input level — the strands *are* the meter. This
satisfies the "live waveform" interaction pattern below without adding a
second waveform widget.

> **Dependency:** the frontend has no audio level today. The only event the
> backend emits is `transcript:segment`. A live meter needs a new
> low-frequency level event (~20 Hz, a single smoothed peak or RMS float)
> emitted from the capture path. Until that exists, the strands animate on
> a fixed idle loop while recording — honest as "we are listening", but not
> a real meter.

**Accessibility.** The button is a real `<button>` with an
`aria-pressed` state and a text label — never an icon alone. The pulse and
strand animation are decorative: recording state is *also* carried by the
label text and the timer, so nothing is lost when animation is disabled.
Under `prefers-reduced-motion` the ring is a solid recording colour with no
pulse and the strands hold a static extended pose.

---

## Layout `[partly built]`

Built today: the single centred column that fills the viewport, the action
bar pinned to its bottom, `overflow-wrap: anywhere` on transcript text, and
44×44px header controls. The column caps at 640px rather than 480px, and
the breakpoint table below is still `[spec]` — the responsive system is #78.

**One layout tree for desktop and mobile.** A single column, `max-width:
480px`, centred, with a fixed action bar at the bottom. Phones get it
full-bleed; desktop gets it centred with breathing room. There is no second
design to keep in sync — which matters for a solo project.

Only one breakpoint does anything structural:

| Width | Behaviour |
|---|---|
| < 480px | Full-bleed column. `env(safe-area-inset-*)` padding on the action bar. |
| ≥ 480px | Column caps at 480px and centres. |
| ≥ 900px | History promotes from a sheet to a persistent left rail (280px). Everything else is unchanged. |

Rules that hold at every width: touch targets ≥44×44px (today's buttons are
~38px); no horizontal scrolling ever; text wraps with `overflow-wrap:
anywhere` (transcripts contain arbitrary user speech in arbitrary scripts).

---

## Key Screens `[partly built]`

The app used to be one scrolling page of seven stacked sections, with
**Recent Sessions above the live Transcript** — while you were recording,
you watched a list of old sessions while your words rendered off-screen
below. Record (below) fixes that inversion; Settings is still `[spec]`.

### Record — home `[built]`

Transcript flows upward, newest at the bottom, like a chat log. Everyone
already knows how to read this.

Shipped in #72. The two interim compromises noted in earlier drafts — History
taking over the main area instead of being a sheet, and translation living
under the transcript instead of on the session — were resolved by #73 and
#74 respectively.

```
┌──────────────────────────────┐
│ 🖌 PolyVocal          ⚙   ☾  │  mark + wordmark lockup
├──────────────────────────────┤
│                              │
│  "Aprendi a falar alguma     │  ← transcript, --text-xl
│   coisa."                    │
│                              │
│  "E depois disso, mais       │
│   nada."                     │
│                        ▲     │  auto-scrolls, newest at bottom
├──────────────────────────────┤
│  Português → English    ⌄    │  language bar, set once
│                              │
│         ((( 🖌 )))           │  ← 88px, strands live
│                              │
│      0:05 · Tap to stop      │
└──────────────────────────────┘
```

Empty state replaces the transcript area with one line — *"Tap the brush
and start talking."* — and nothing else. No cards, no tips, no onboarding
carousel.

### Session — after stopping, and from History `[built]`

Translation lives here as a toggle on the session, not as a separate
section with its own button at the bottom of the page.

```
┌──────────────────────────────┐
│ ←  Aug 14, 19:11        ⋯    │  ⋯ = export, delete
├──────────────────────────────┤
│  Português · 5s              │
│                              │
│  "Aprendi a falar alguma     │
│   coisa."                    │
│                              │
├──────────────────────────────┤
│  [ Original | English ⌄ ]    │  ← one tap translates the session
└──────────────────────────────┘
```

Export and delete live **inside** the opened session, in the `⋯` menu.
Previously they sat side by side on every row of the list, which both
squeezed badly at 360px and put a destructive action one mistap away while
scrolling. Shipped in #74, opening from the record button after `stop_recording`
and from a History card, and nesting over an already-open History sheet when
opened the second way.

### History — sheet, or left rail ≥900px

```
┌──────────────────────────────┐
│ ←  History                   │
├──────────────────────────────┤
│  ┌────────────────────────┐  │
│  │ "Aprendi a falar…"     │  │
│  │ PT → EN · 5s · today   │  │
│  └────────────────────────┘  │
│  ┌────────────────────────┐  │
│  │ "Boa tarde, tudo bem?" │  │
│  │ PT · 12s · yesterday   │  │
│  └────────────────────────┘  │
└──────────────────────────────┘
```

Whole card is the tap target. Language pair is a chip, not a sentence.

### Settings — sheet

```
┌──────────────────────────────┐
│ ←  Settings                  │
├──────────────────────────────┤
│  LANGUAGES                   │
│  ✓ Portuguese ↔ English      │
│    489 MB · ready            │
│  ↓ Spanish ↔ English         │
│    312 MB · download         │
│                              │
│  ACCURACY                    │
│  ( ) Fast      75 MB         │
│  (•) Balanced  466 MB        │
│  ( ) Best      1.5 GB        │
│                              │
│  Microphone   [ Default ⌄ ]  │
│  Appearance   [ Auto ⌄ ]     │
└──────────────────────────────┘
```

Whisper model sizes become **Fast / Balanced / Best** with their download
cost attached. "tiny/base/small/medium" is vocabulary for people who
already know what a Whisper model is.

---

## Interaction Patterns

- **Toggle record** `[built]` — tap to start, tap to stop. Push-to-talk and
  a settings choice between the two remain unbuilt.
- **Live waveform** `[spec]` — carried by the record button's strands.
  Blocked on the audio level event described above.
- **Inline translation** `[built]` — a `[ Original | English ⌄ ]` toggle on
  the session, translating in place with the same one-shot `translate_text`
  call the old bottom-of-page control made.
- **Sheets, not pages** `[spec]` — History and Settings slide over the
  record screen and dismiss back to it. The record screen is always the
  thing underneath; you can never get lost.

---

## Accessibility `[partly built]`

Built today: OS-native font stack that respects user text scaling, a
three-state theme toggle, `prefers-reduced-motion` on the recording dot,
visible focus rings on buttons and selects, icons that are all
`aria-hidden` with a paired text label — so no icon carries meaning alone —
`aria-live="polite"` on the transcript region so live segments are
announced, `role="alert"` on the error line, `aria-pressed` on the record
toggle reflecting recording state, and 44×44px minimum touch targets on the
icon-only header controls and the record button.

Required by this design and **not** built today:

- Focus must move into a sheet when it opens, be trapped while open, and
  return to the invoking control on close.

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
