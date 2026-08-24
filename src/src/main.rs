use std::time::Duration;

use futures::StreamExt;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;

/// Key the manual theme override is persisted under in `localStorage`.
const THEME_STORAGE_KEY: &str = "polyvocal-theme";

/// Theme selection. `Auto` (the default) defers entirely to the OS via the
/// `prefers-color-scheme` CSS media query — no `data-theme` attribute is
/// set. `Light`/`Dark` are an explicit manual override, applied via
/// `data-theme` on `<html>` and persisted so it survives a reload.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ThemeMode {
    Auto,
    Light,
    Dark,
}

impl ThemeMode {
    fn storage_value(self) -> Option<&'static str> {
        match self {
            ThemeMode::Auto => None,
            ThemeMode::Light => Some("light"),
            ThemeMode::Dark => Some("dark"),
        }
    }

    fn from_storage_value(value: &str) -> Self {
        match value {
            "light" => ThemeMode::Light,
            "dark" => ThemeMode::Dark,
            _ => ThemeMode::Auto,
        }
    }

    fn label(self) -> &'static str {
        match self {
            ThemeMode::Auto => "Match system",
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
        }
    }
}

/// Reads the persisted manual override, if any. Falls back to `Auto`
/// (letting `prefers-color-scheme` decide) when nothing is stored yet.
fn stored_theme_mode() -> ThemeMode {
    window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|storage| storage.get_item(THEME_STORAGE_KEY).ok().flatten())
        .map(|value| ThemeMode::from_storage_value(&value))
        .unwrap_or(ThemeMode::Auto)
}

/// Key the record-toggle keyboard shortcut is persisted under in
/// `localStorage`.
const RECORD_SHORTCUT_STORAGE_KEY: &str = "polyvocal-record-shortcut";

/// The key that toggles recording (see `App`'s `window_event_listener`).
/// A local accelerator only — scoped to the app window and only live while
/// the record screen is showing, not a `tauri-plugin-global-shortcut`
/// registration (see #125).
#[derive(Clone, Copy, PartialEq, Eq)]
enum RecordShortcutKey {
    Space,
    R,
    S,
}

impl RecordShortcutKey {
    /// The `KeyboardEvent.code` value to match against — layout-independent,
    /// unlike `.key()`.
    fn code(self) -> &'static str {
        match self {
            RecordShortcutKey::Space => "Space",
            RecordShortcutKey::R => "KeyR",
            RecordShortcutKey::S => "KeyS",
        }
    }

    fn storage_value(self) -> &'static str {
        match self {
            RecordShortcutKey::Space => "space",
            RecordShortcutKey::R => "r",
            RecordShortcutKey::S => "s",
        }
    }

    fn from_storage_value(value: &str) -> Self {
        match value {
            "r" => RecordShortcutKey::R,
            "s" => RecordShortcutKey::S,
            _ => RecordShortcutKey::Space,
        }
    }

    fn label(self) -> &'static str {
        match self {
            RecordShortcutKey::Space => "Space",
            RecordShortcutKey::R => "R",
            RecordShortcutKey::S => "S",
        }
    }
}

/// Reads the persisted record-shortcut key. Falls back to `Space`.
fn stored_record_shortcut() -> RecordShortcutKey {
    window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|storage| storage.get_item(RECORD_SHORTCUT_STORAGE_KEY).ok().flatten())
        .map(|value| RecordShortcutKey::from_storage_value(&value))
        .unwrap_or(RecordShortcutKey::Space)
}

/// Applies a theme mode to the document and persists it. `Auto` clears the
/// override entirely so the `prefers-color-scheme` media query in
/// `styles.css` takes over.
fn apply_theme_mode(mode: ThemeMode) {
    if let Some(root) = document().document_element() {
        match mode.storage_value() {
            Some(value) => {
                let _ = root.set_attribute("data-theme", value);
            }
            None => {
                let _ = root.remove_attribute("data-theme");
            }
        }
    }
    if let Ok(Some(storage)) = window().local_storage() {
        match mode.storage_value() {
            Some(value) => {
                let _ = storage.set_item(THEME_STORAGE_KEY, value);
            }
            None => {
                let _ = storage.remove_item(THEME_STORAGE_KEY);
            }
        }
    }
}

/// Persists the record-toggle shortcut key choice.
fn apply_record_shortcut(key: RecordShortcutKey) {
    if let Ok(Some(storage)) = window().local_storage() {
        let _ = storage.set_item(RECORD_SHORTCUT_STORAGE_KEY, key.storage_value());
    }
}

/// Minimal inline icons, adapted from Lucide (ISC license — ../design/DESIGN.md).
/// Stroke-only, `currentColor`, and `aria-hidden` since every icon here is
/// paired with a text label — icons never carry meaning on their own.
mod icons {
    use leptos::prelude::*;

    #[component]
    pub fn Languages() -> impl IntoView {
        view! {
            <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="m5 8 6 6"/>
                <path d="m4 14 6-6 2-3"/>
                <path d="M2 5h12"/>
                <path d="M7 2h1"/>
                <path d="m22 22-5-10-5 10"/>
                <path d="M14 18h6"/>
            </svg>
        }
    }

    #[component]
    pub fn TriangleAlert() -> impl IntoView {
        view! {
            <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3"/>
                <path d="M12 9v4"/>
                <path d="M12 17h.01"/>
            </svg>
        }
    }

    #[component]
    pub fn History() -> impl IntoView {
        view! {
            <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/>
                <path d="M3 3v5h5"/>
                <path d="M12 7v5l4 2"/>
            </svg>
        }
    }

    #[component]
    pub fn ArrowLeft() -> impl IntoView {
        view! {
            <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="m12 19-7-7 7-7"/>
                <path d="M19 12H5"/>
            </svg>
        }
    }

    #[component]
    pub fn MoreHorizontal() -> impl IntoView {
        view! {
            <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <circle cx="12" cy="12" r="1"/>
                <circle cx="19" cy="12" r="1"/>
                <circle cx="5" cy="12" r="1"/>
            </svg>
        }
    }

    #[component]
    pub fn Trash2() -> impl IntoView {
        view! {
            <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M3 6h18"/>
                <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/>
                <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                <line x1="10" x2="10" y1="11" y2="17"/>
                <line x1="14" x2="14" y1="11" y2="17"/>
            </svg>
        }
    }

    #[component]
    pub fn Settings() -> impl IntoView {
        view! {
            <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
                <circle cx="12" cy="12" r="3"/>
            </svg>
        }
    }
}
use icons::{ArrowLeft, History, Languages, MoreHorizontal, Settings, Trash2, TriangleAlert};

/// The PolyVocal mark — the hairbrush, rebuilt as inline SVG so it themes,
/// stays crisp at any size, and exposes each strand as its own path. Geometry
/// is specified in `../design/DESIGN.md` → *The PolyVocal Mark*: a `0 0 32 32`
/// grid, the brush body drawn upright and rotated as one group so the tilt is
/// a single number, the strands springing from the paddle's right edge.
///
/// Colours come from CSS custom properties, never `currentColor` — the mark is
/// polychrome and must not inherit the surrounding text colour. The strands
/// live in their own `.pv-strands` group so #71 can animate them individually.
///
/// `size` is the rendered edge in px, and drives the detail variant:
///
/// | Size | Detail |
/// |---|---|
/// | > 24px | Everything — paddle, pad, bristles, handle, 4 strands. |
/// | 21–24px | Header lockup: bristles dropped, they'd be sub-pixel mush. |
/// | ≤ 20px | Paddle, handle and 2 strands — 4 strands blur into a blob. |
#[component]
fn PolyVocalMark(#[prop(default = 24)] size: u32) -> impl IntoView {
    let bristles = size > 24;
    let all_strands = size > 20;
    view! {
        <svg
            class="pv-mark"
            width=size
            height=size
            viewBox="0 0 32 32"
            fill="none"
            aria-hidden="true"
        >
            <g transform="rotate(-15 16 16)">
                <rect x="9.75" y="16" width="4.5" height="10" rx="2.25" fill="var(--color-primary)"/>
                <circle cx="12" cy="26" r="3" fill="var(--color-primary)"/>
                <rect x="7" y="3.5" width="12" height="15" rx="6" fill="var(--color-primary)"/>
                <rect x="8.75" y="5.25" width="8.5" height="11.5" rx="4.25" fill="var(--color-primary-shade)"/>
                {bristles.then(|| view! {
                    <g stroke="var(--color-primary)" stroke-width="1" stroke-linecap="round">
                        <path d="M11.5 7.4v1.2"/>
                        <path d="M14.5 7.4v1.2"/>
                        <path d="M11.5 10.4v1.2"/>
                        <path d="M14.5 10.4v1.2"/>
                        <path d="M11.5 13.4v1.2"/>
                        <path d="M14.5 13.4v1.2"/>
                    </g>
                })}
            </g>
            <g class="pv-strands" stroke-width="2" stroke-linecap="round">
                <path d="M17 15 Q25 16 28 12" stroke="var(--strand-4)"/>
                {all_strands.then(|| view! {
                    <path d="M17 13 Q24 12 26  7" stroke="var(--strand-1)"/>
                })}
                <path d="M18 10 Q25 10 29  6" stroke="var(--strand-2)"/>
                {all_strands.then(|| view! {
                    <path d="M18  8 Q24  6 28  4" stroke="var(--strand-3)"/>
                })}
            </g>
        </svg>
    }
}

/// The four record-button states from `../design/DESIGN.md` → *The Record
/// Button*, each with its own ring colour, mark treatment, strand behaviour
/// and label.
///
/// Derived from the two signals the app already has rather than from new
/// backend state. `busy` is set synchronously when the button is clicked,
/// while `recording` only flips once the awaited command returns, so the
/// pair already separates the two in-flight windows:
///
/// | `recording` | `busy` | State | Meaning |
/// |---|---|---|---|
/// | false | false | `Idle` | nothing in flight |
/// | false | true | `Disabled` | `start_recording` is awaiting — capture hasn't opened yet |
/// | true | false | `Recording` | capture is live |
/// | true | true | `Processing` | Stop was pressed; transcription and persistence are finishing |
#[derive(Clone, Copy, PartialEq, Eq)]
enum RecordState {
    Idle,
    Recording,
    Processing,
    Disabled,
}

impl RecordState {
    fn from_signals(recording: bool, busy: bool) -> Self {
        match (recording, busy) {
            (true, true) => RecordState::Processing,
            (true, false) => RecordState::Recording,
            (false, true) => RecordState::Disabled,
            (false, false) => RecordState::Idle,
        }
    }
}

/// Which text the session detail sheet's `[ Original | English ⌄ ]` toggle
/// (`../design/DESIGN.md` → *Key Screens · Session*) is currently showing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SessionView {
    Original,
    Translated,
}

/// `M:SS` for the recording label. Deliberately not zero-padded on the
/// minutes — the label reads "0:05 · Tap to stop", not "00:05".
fn format_elapsed(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// "Aug 14, 19:11" for the session detail header (`../design/DESIGN.md` →
/// *Key Screens · Session*). `created_at` is an RFC 3339 UTC string (backend
/// uses `chrono::Utc::now().to_rfc3339()`); `js_sys::Date` parses that
/// directly and its plain (non-UTC) getters convert to the OS local time
/// zone for free — a timestamp is more useful read in the time the user
/// actually experienced it than in UTC.
fn format_session_datetime(created_at: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(created_at));
    if date.get_time().is_nan() {
        return created_at.to_string();
    }
    let month = date.get_month() as usize;
    let Some(month_name) = MONTHS.get(month) else {
        return created_at.to_string();
    };
    let day = date.get_date();
    let hours = date.get_hours();
    let minutes = date.get_minutes();
    format!("{month_name} {day}, {hours:02}:{minutes:02}")
}

/// "5s" for a short session, "1:05" (reusing `format_elapsed`) once it runs
/// past a minute — the session detail meta line's duration, per the "5s" in
/// `../design/DESIGN.md` → *Key Screens · Session*.
fn format_duration_label(duration_ms: i64) -> String {
    let secs = (duration_ms.max(0) / 1000) as u32;
    if secs < 60 {
        format!("{secs}s")
    } else {
        format_elapsed(secs)
    }
}

/// Mirrors the `transcript:segment` event payload emitted by the Rust
/// backend (DEC-007) — only the fields this screen renders are declared;
/// serde ignores the rest.
#[derive(Deserialize, Clone)]
struct TranscriptSegment {
    text: String,
    language: String,
}

/// Mirrors the `audio:level` event payload emitted by the Rust backend
/// (#76) — a single smoothed RMS amplitude in `[0, 1]`, sampled from the mic
/// roughly 20 times a second while recording.
#[derive(Deserialize, Clone)]
struct AudioLevel {
    level: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StartRecordingArgs {
    device_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TranslateArgs<'a> {
    session_id: &'a str,
    target_lang: &'a str,
}

/// Mirrors the backend `storage::models::Session` struct — only the fields
/// this screen renders are declared; serde ignores the rest (same approach
/// as `TranscriptSegment` above). No `#[serde(rename_all)]` here — unlike
/// the `*Args` structs above (which are command *arguments*, auto-camelCased
/// by Tauri), this is a command *return value*, serialized as-is by the
/// backend's own (unrenamed, snake_case) `Serialize` impl.
#[derive(Deserialize, Clone)]
struct Session {
    id: String,
    created_at: String,
    duration_ms: i64,
    language: Option<String>,
    transcript: String,
    translation: Option<String>,
    target_lang: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListSessionsArgs {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GetSessionArgs<'a> {
    id: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeleteSessionArgs<'a> {
    id: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportSessionTxtArgs<'a> {
    id: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportSessionSrtArgs<'a> {
    id: &'a str,
}

/// How many of the (newest-first) sessions the history list shows before the
/// user asks for the rest. Keeps "Recent Sessions" from pushing the live
/// transcript and translation off the bottom of the screen.
const SESSION_PREVIEW_COUNT: usize = 3;

/// Character budget for a session card's transcript preview before it's
/// truncated with a trailing "…".
const SESSION_PREVIEW_CHAR_LIMIT: usize = 80;

/// Truncates `transcript` to at most `max_chars` characters, appending "…"
/// when it was actually cut short. Counts by `char`, not byte, so this is
/// safe on multi-byte UTF-8 transcripts (translated/non-English sessions).
fn truncate_preview(transcript: &str, max_chars: usize) -> String {
    if transcript.chars().count() > max_chars {
        let truncated: String = transcript.chars().take(max_chars).collect();
        format!("{truncated}…")
    } else {
        transcript.to_string()
    }
}

/// MVP language pairs, matching `translation::SUPPORTED_LANGUAGES` in the backend.
const TARGET_LANGUAGES: [(&str, &str); 3] =
    [("en", "English"), ("pt", "Portuguese"), ("es", "Spanish")];

/// Human name for an ISO code, for the action bar's language pill —
/// "Languages, not files" (`../design/DESIGN.md` → *Design Principles*).
/// Whisper detects far more languages than the three we can translate
/// between, so anything outside `TARGET_LANGUAGES` falls back to the raw
/// code rather than being hidden.
fn language_label(code: &str) -> String {
    TARGET_LANGUAGES
        .iter()
        .find(|(candidate, _)| *candidate == code)
        .map(|(_, label)| (*label).to_string())
        .unwrap_or_else(|| code.to_uppercase())
}

/// Mirrors `models::registry::ModelSize` — `rename_all = "lowercase"` on the
/// backend, so this must serialize/deserialize the same way to match.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum ModelSize {
    Tiny,
    Base,
    Small,
    Medium,
}

impl ModelSize {
    /// "Fast/Balanced/Best" rather than "tiny/base/small/medium" —
    /// `../../design/DESIGN.md` principle 5 ("Languages, not files") reads
    /// the same way for Whisper sizes: "tiny/base/small/medium" is
    /// vocabulary for people who already know what a Whisper model is.
    /// `Base` is a real, selectable backend size but sits between Fast and
    /// Balanced with no room in a three-way picker, so it's the one size
    /// Settings doesn't surface — a judgment call, not a backend removal
    /// (see `MODEL_PICKER_SIZES` below).
    fn label(self) -> &'static str {
        match self {
            ModelSize::Tiny => "Fast",
            ModelSize::Base => "Base",
            ModelSize::Small => "Balanced",
            ModelSize::Medium => "Best",
        }
    }

    /// Matches `models::registry::ModelSize::size_mb` in the backend.
    fn size_mb(self) -> u32 {
        match self {
            ModelSize::Tiny => 75,
            ModelSize::Base => 145,
            ModelSize::Small => 465,
            ModelSize::Medium => 1500,
        }
    }
}

/// The three sizes Settings' "Accuracy" picker offers — see
/// `ModelSize::label`'s doc comment on why `Base` is excluded.
const MODEL_PICKER_SIZES: [ModelSize; 3] = [ModelSize::Tiny, ModelSize::Small, ModelSize::Medium];

/// Mirrors `models::registry::ModelInfo` — a command *return value*,
/// serialized as-is by the backend's own (unrenamed) `Serialize` impl, so
/// no `#[serde(rename_all)]` here — unlike the `*Args` structs below, which
/// are command *arguments* and get auto-camelCased by Tauri.
#[derive(Deserialize, Clone)]
struct ModelInfo {
    size: ModelSize,
    downloaded: bool,
    is_active: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadModelArgs {
    size: ModelSize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SetActiveModelArgs {
    size: ModelSize,
}

/// Mirrors `commands::translation::LanguagePairInfo` — a command *return
/// value*, serialized as-is by the backend's own (unrenamed) `Serialize`
/// impl, same reasoning as `ModelInfo` above.
#[derive(Deserialize, Clone)]
struct LanguagePairInfo {
    language: String,
    size_mb: u32,
    downloaded: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadTranslationModelArgs {
    language: String,
}

/// Mirrors `audio::InputDevice` — a command return value, same reasoning as
/// `ModelInfo` above.
#[derive(Deserialize, Clone)]
struct InputDevice {
    id: String,
    name: String,
    is_default: bool,
}

/// Every element type a keyboard user can land on, for the sheet's focus
/// trap below. `:not([disabled])` matters — a disabled button is still
/// matched by `button` alone but isn't reachable by Tab.
const FOCUSABLE_SELECTOR: &str = "a[href], button:not([disabled]), \
    textarea:not([disabled]), input:not([disabled]), select:not([disabled]), \
    [tabindex]:not([tabindex='-1'])";

/// Queries `container` for everything the focus trap needs to cycle
/// between. Silently empty on a query failure (malformed selector, which
/// can't happen here) or when nothing inside is reachable — callers treat
/// an empty list as "nothing to trap yet".
fn focusable_elements(container: &web_sys::HtmlDivElement) -> Vec<web_sys::HtmlElement> {
    let Ok(list) = container.query_selector_all(FOCUSABLE_SELECTOR) else {
        return Vec::new();
    };
    (0..list.length())
        .filter_map(|i| list.item(i))
        .filter_map(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
        .collect()
}

/// A screen that slides over the record screen and dismisses back to it
/// (`../design/DESIGN.md` → *Key Screens*, *Interaction Patterns · Sheets,
/// not pages*). Always mounted — `open` only toggles a CSS class — so the
/// close transition has something to animate and so the focus-management
/// effect below can observe both edges of the open/close transition rather
/// than only ever seeing "just mounted, already open".
///
/// Handles the three behaviours DESIGN.md and #73 require of every sheet:
/// focus moves in on open, is trapped by Tab/Shift+Tab while open, and
/// returns to `invoker` (the control that opened it) on close — via
/// Escape, the backdrop, or the in-sheet back control.
///
/// `title` and `invoker` are reactive (`Signal`, not a plain value) so a
/// sheet whose header text or opening control depends on which session was
/// tapped — the session detail sheet added in #74 — can still fit this same
/// component: Settings and History just wrap a constant in `Signal::derive`.
/// `header_extra` is a slot rendered after the back button, for the session
/// detail sheet's `⋯` menu; Settings and History pass an empty closure since
/// there's nothing there for either of them.
#[component]
fn Sheet(
    open: Signal<bool>,
    on_close: Callback<()>,
    title: Signal<String>,
    variant: &'static str,
    invoker: Signal<Option<web_sys::HtmlElement>>,
    header_extra: ChildrenFn,
    children: ChildrenFn,
) -> impl IntoView {
    let panel_ref = NodeRef::<leptos::html::Div>::new();
    let back_ref = NodeRef::<leptos::html::Button>::new();

    // Fires only on the open/close edges (via the `prev` accumulator), not
    // on every reactive re-run, so it doesn't fight the user's own focus
    // once the sheet has settled open. `invoker` is read untracked — its
    // value can change while the sheet stays open (a different session
    // card next time) without that alone re-running this effect.
    Effect::new(move |prev: Option<bool>| {
        let now_open = open.get();
        if now_open && prev != Some(true) {
            // A frame late, same reason as the transcript autoscroll effect:
            // the panel's `is-open` class (and thus its visibility) lands in
            // the DOM after this effect runs, and a hidden element can't
            // take focus.
            request_animation_frame(move || {
                if let Some(back) = back_ref.get() {
                    let _ = back.focus();
                }
            });
        } else if !now_open && prev == Some(true) {
            if let Some(invoker) = invoker.get_untracked() {
                let _ = invoker.focus();
            }
        }
        now_open
    });

    let on_keydown = move |ev: web_sys::KeyboardEvent| match ev.key().as_str() {
        "Escape" => on_close.run(()),
        "Tab" => {
            let Some(panel) = panel_ref.get() else {
                return;
            };
            let focusables = focusable_elements(&panel);
            let (Some(first), Some(last)) = (focusables.first(), focusables.last()) else {
                return;
            };
            let active = document().active_element();
            if ev.shift_key() {
                if first.is_same_node(active.as_deref()) {
                    ev.prevent_default();
                    let _ = last.focus();
                }
            } else if last.is_same_node(active.as_deref()) {
                ev.prevent_default();
                let _ = first.focus();
            }
        }
        _ => {}
    };

    view! {
        <div
            class=move || format!("sheet-backdrop {variant}-backdrop{}", if open.get() { " is-open" } else { "" })
            on:click=move |_| on_close.run(())
        ></div>
        <div
            class=move || format!("sheet {variant}{}", if open.get() { " is-open" } else { "" })
            node_ref=panel_ref
            role="dialog"
            aria-modal="true"
            aria-label=move || title.get()
            aria-hidden=move || (!open.get()).to_string()
            on:keydown=on_keydown
        >
            <div class="sheet-header">
                <button class="sheet-back" node_ref=back_ref on:click=move |_| on_close.run(())>
                    <ArrowLeft/>
                    <span>{move || title.get()}</span>
                </button>
                {header_extra()}
            </div>
            <div class="sheet-body">{children()}</div>
        </div>
    }
}

/// The session list, shared verbatim between the History sheet (< 900px)
/// and the persistent History rail (>= 900px, `../design/DESIGN.md` →
/// *Layout*) rather than duplicated — the two are the same content in two
/// different shells, never two designs to keep in sync.
///
/// The whole card is the tap target (`../design/DESIGN.md` → *Key Screens ·
/// History*: "Whole card is the tap target") — export moved into the opened
/// session's `⋯` menu in #74. Delete also got a hover-reveal shortcut
/// directly on the card in #126 (a `:focus-within`/`:focus-visible`-gated
/// `.session-card-delete` button, sibling to the card so it isn't nested
/// inside another `<button>`), since the sheet round-trip was too much
/// friction for a common destructive action — full delete still lives in
/// the session detail `⋯` menu too.
#[component]
fn SessionList(
    sessions: RwSignal<Vec<Session>>,
    sessions_loading: RwSignal<bool>,
    sessions_expanded: RwSignal<bool>,
    on_open: Callback<(String, web_sys::HtmlElement)>,
    error_message: RwSignal<Option<String>>,
) -> impl IntoView {
    // Only one card can be in the "confirm delete?" state at a time — mirrors
    // the session-detail sheet's own `pending_delete` signal.
    let card_pending_delete = RwSignal::new(None::<String>);

    view! {
        {move || {
            if sessions_loading.get() {
                view! { <p class="sessions-empty">"Loading…"</p> }.into_any()
            } else if sessions.get().is_empty() {
                view! { <p class="sessions-empty">"No sessions yet."</p> }.into_any()
            } else {
                let all = sessions.get();
                let total = all.len();
                let visible: Vec<Session> = if sessions_expanded.get() {
                    all
                } else {
                    all.into_iter().take(SESSION_PREVIEW_COUNT).collect()
                };
                view! {
                    <ul class="session-list">
                        {visible.into_iter().map(|session| {
                            let id = session.id.clone();
                            let preview = truncate_preview(&session.transcript, SESSION_PREVIEW_CHAR_LIMIT);
                            let language_label = session.language.clone().unwrap_or_else(|| "—".to_string());
                            let created_at = session.created_at.clone();
                            let translation_note = if session.translation.is_some() {
                                let target = session.target_lang.clone().unwrap_or_default();
                                format!(" · → {target}")
                            } else {
                                String::new()
                            };
                            let open_id = id.clone();
                            let delete_id = id.clone();
                            let is_confirming_id = id.clone();
                            let label_id = id.clone();
                            view! {
                                <li class="session-item">
                                    <button
                                        class="session-card"
                                        on:click=move |ev| {
                                            if let Some(target) = ev.current_target().and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok()) {
                                                on_open.run((open_id.clone(), target));
                                            }
                                        }
                                    >
                                        <p class="session-preview">{preview}</p>
                                        <p class="session-meta">{language_label}" · "{created_at}{translation_note}</p>
                                    </button>
                                    <button
                                        class="session-card-delete"
                                        class:is-confirming=move || card_pending_delete.get().as_deref() == Some(is_confirming_id.as_str())
                                        aria-label=move || if card_pending_delete.get().as_deref() == Some(label_id.as_str()) {
                                            "Confirm delete session"
                                        } else {
                                            "Delete session"
                                        }
                                        on:click=move |_| {
                                            if card_pending_delete.get_untracked().as_deref() == Some(delete_id.as_str()) {
                                                let id = delete_id.clone();
                                                card_pending_delete.set(None);
                                                spawn_local(async move {
                                                    let args = DeleteSessionArgs { id: &id };
                                                    match tauri_sys::core::invoke_result::<(), String>("delete_session", args).await {
                                                        Ok(()) => {
                                                            sessions.update(|list| list.retain(|s| s.id != id));
                                                            error_message.set(None);
                                                        }
                                                        Err(e) => error_message.set(Some(e)),
                                                    }
                                                });
                                            } else {
                                                card_pending_delete.set(Some(delete_id.clone()));
                                            }
                                        }
                                    >
                                        <Trash2/>
                                    </button>
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                    {(total > SESSION_PREVIEW_COUNT).then(|| view! {
                        <button
                            class="sessions-toggle"
                            on:click=move |_| sessions_expanded.update(|open| *open = !*open)
                        >
                            {move || if sessions_expanded.get() {
                                "Show less".to_string()
                            } else {
                                format!("View all {total} sessions")
                            }}
                        </button>
                    })}
                }.into_any()
            }
        }}
    }
}

/// The session detail screen (`../design/DESIGN.md` → *Key Screens ·
/// Session*) — opened after stopping a recording and from a History card
/// (`session_detail_id` is set from both places in `App`). Built on the
/// shared `Sheet`: a dynamic title (the formatted timestamp), a dynamic
/// invoker (whichever record button or session card opened it), and the
/// `⋯` export/delete menu in `header_extra`.
///
/// Fetches the full `Session` itself via `get_session` on open rather than
/// relying on the (truncated-preview, translation-less-if-untranslated)
/// copy already in the shared `sessions` list, since that list is what's
/// updated (or pruned, on delete) once this sheet acts on the session.
#[component]
fn SessionDetailSheet(
    session_detail_id: RwSignal<Option<String>>,
    invoker: Signal<Option<web_sys::HtmlElement>>,
    sessions: RwSignal<Vec<Session>>,
    default_target_lang: RwSignal<String>,
    error_message: RwSignal<Option<String>>,
) -> impl IntoView {
    let detail = RwSignal::new(None::<Session>);
    let loading = RwSignal::new(false);
    let view_mode = RwSignal::new(SessionView::Original);
    let target_lang = RwSignal::new(default_target_lang.get_untracked());
    let translating = RwSignal::new(false);
    let menu_open = RwSignal::new(false);
    let pending_delete = RwSignal::new(false);

    // Refetches only on an actual id change (open with a new/different
    // session), not on every reactive rerun — same edge-detection shape as
    // Sheet's own focus effect above.
    Effect::new(move |prev: Option<Option<String>>| {
        let id = session_detail_id.get();
        let changed = id != prev.clone().unwrap_or(None);
        if changed {
            if let Some(id) = id.clone() {
                detail.set(None);
                view_mode.set(SessionView::Original);
                menu_open.set(false);
                pending_delete.set(false);
                target_lang.set(default_target_lang.get_untracked());
                loading.set(true);
                spawn_local(async move {
                    let args = GetSessionArgs { id: &id };
                    match tauri_sys::core::invoke_result::<Option<Session>, String>(
                        "get_session",
                        args,
                    )
                    .await
                    {
                        Ok(Some(session)) => {
                            // A session that was already translated before
                            // (e.g. reopened from History) should show the
                            // toggle pointed at *that* language, not silently
                            // fall back to the app-wide default while still
                            // displaying the old translation underneath.
                            if let Some(lang) = session.target_lang.clone() {
                                target_lang.set(lang);
                            }
                            detail.set(Some(session));
                        }
                        Ok(None) => error_message.set(Some("Session not found.".to_string())),
                        Err(e) => error_message.set(Some(e)),
                    }
                    loading.set(false);
                });
            }
        }
        id
    });

    // The toggle's one backend call (`../design/DESIGN.md` → *Key Screens ·
    // Session*: "one tap translates the session"). A translation already
    // cached on `detail` for `lang` just switches the view; otherwise this
    // is the same one-shot `translate_text` call the old bottom-of-page
    // control made, persisted onto `detail` so re-toggling is instant.
    let translate_now = move |lang: String| {
        target_lang.set(lang.clone());
        let cached = detail.get_untracked().is_some_and(|s| {
            s.translation.is_some() && s.target_lang.as_deref() == Some(lang.as_str())
        });
        if cached {
            view_mode.set(SessionView::Translated);
            return;
        }
        let Some(id) = session_detail_id.get_untracked() else {
            return;
        };
        translating.set(true);
        error_message.set(None);
        spawn_local(async move {
            let args = TranslateArgs {
                session_id: &id,
                target_lang: &lang,
            };
            match tauri_sys::core::invoke_result::<String, String>("translate_text", args).await {
                Ok(text) => {
                    detail.update(|maybe| {
                        if let Some(session) = maybe {
                            session.translation = Some(text);
                            session.target_lang = Some(lang);
                        }
                    });
                    view_mode.set(SessionView::Translated);
                }
                Err(e) => error_message.set(Some(e)),
            }
            translating.set(false);
        });
    };

    let export_txt = move |_| {
        menu_open.set(false);
        let Some(id) = session_detail_id.get_untracked() else {
            return;
        };
        spawn_local(async move {
            let args = ExportSessionTxtArgs { id: &id };
            match tauri_sys::core::invoke_result::<Option<String>, String>(
                "export_session_txt",
                args,
            )
            .await
            {
                Ok(_) => error_message.set(None),
                Err(e) => error_message.set(Some(e)),
            }
        });
    };

    let export_srt = move |_| {
        menu_open.set(false);
        let Some(id) = session_detail_id.get_untracked() else {
            return;
        };
        spawn_local(async move {
            let args = ExportSessionSrtArgs { id: &id };
            match tauri_sys::core::invoke_result::<Option<String>, String>(
                "export_session_srt",
                args,
            )
            .await
            {
                Ok(_) => error_message.set(None),
                Err(e) => error_message.set(Some(e)),
            }
        });
    };

    let delete_now = move |_| {
        let Some(id) = session_detail_id.get_untracked() else {
            return;
        };
        if pending_delete.get_untracked() {
            pending_delete.set(false);
            spawn_local(async move {
                let args = DeleteSessionArgs { id: &id };
                match tauri_sys::core::invoke_result::<(), String>("delete_session", args).await {
                    Ok(()) => {
                        sessions.update(|list| list.retain(|s| s.id != id));
                        session_detail_id.set(None);
                    }
                    Err(e) => error_message.set(Some(e)),
                }
            });
        } else {
            pending_delete.set(true);
        }
    };

    view! {
        <Sheet
            open=Signal::derive(move || session_detail_id.get().is_some())
            on_close=Callback::new(move |_| session_detail_id.set(None))
            title=Signal::derive(move || {
                detail.get().map(|s| format_session_datetime(&s.created_at)).unwrap_or_default()
            })
            variant="session-detail-sheet"
            invoker=invoker
            header_extra=std::sync::Arc::new(move || view! {
                <div class="session-menu-wrap">
                    <button
                        class="session-menu-toggle"
                        aria-label="Session menu"
                        aria-expanded=move || if menu_open.get() { "true" } else { "false" }
                        title="Session menu"
                        on:click=move |_| menu_open.update(|open| *open = !*open)
                    >
                        <MoreHorizontal/>
                    </button>
                    <div class="session-menu" class:is-open=move || menu_open.get()>
                        <button class="session-menu-item" on:click=export_txt>"Export TXT"</button>
                        <button class="session-menu-item" on:click=export_srt>"Export SRT"</button>
                        <button
                            class="session-menu-item session-menu-delete"
                            class:is-confirming=move || pending_delete.get()
                            on:click=delete_now
                        >
                            {move || if pending_delete.get() { "Confirm delete?" } else { "Delete" }}
                        </button>
                    </div>
                </div>
            }.into_any())
        >
            {move || {
                if loading.get() {
                    view! { <p class="sessions-empty">"Loading…"</p> }.into_any()
                } else if let Some(session) = detail.get() {
                    let language = session.language.clone().unwrap_or_else(|| "—".to_string());
                    let duration = format_duration_label(session.duration_ms);
                    let original_text = session.transcript.clone();
                    let translated_text = session.translation.clone().unwrap_or_default();
                    view! {
                        <div class="session-detail">
                            <p class="session-detail-meta">{language}" · "{duration}</p>
                            <div class="session-detail-text">
                                {move || match view_mode.get() {
                                    SessionView::Original => original_text.clone(),
                                    SessionView::Translated => translated_text.clone(),
                                }}
                            </div>
                            {move || translating.get().then(|| view! {
                                <p class="translate-status">"Running locally — usually a few seconds, longer the first time a language pair's model needs downloading."</p>
                            })}
                            <div class="translate-toggle" role="group" aria-label="Session view">
                                <button
                                    class="toggle-segment"
                                    class:is-active=move || view_mode.get() == SessionView::Original
                                    on:click=move |_| view_mode.set(SessionView::Original)
                                >
                                    "Original"
                                </button>
                                <select
                                    class="toggle-segment toggle-target"
                                    class:is-active=move || view_mode.get() == SessionView::Translated
                                    aria-label="Translate into"
                                    prop:value=move || target_lang.get()
                                    disabled=move || translating.get()
                                    on:mousedown=move |_| translate_now(target_lang.get_untracked())
                                    on:change=move |ev| translate_now(event_target_value(&ev))
                                >
                                    {TARGET_LANGUAGES
                                        .iter()
                                        .map(|(code, label)| view! { <option value=*code>{*label}</option> })
                                        .collect_view()}
                                </select>
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! { <p class="sessions-empty">"Session not found."</p> }.into_any()
                }
            }}
        </Sheet>
    }
}

#[component]
fn App() -> impl IntoView {
    let theme_mode = RwSignal::new(stored_theme_mode());
    Effect::new(move |_| apply_theme_mode(theme_mode.get()));

    let record_shortcut = RwSignal::new(stored_record_shortcut());
    Effect::new(move |_| apply_record_shortcut(record_shortcut.get()));

    let recording = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let record_state = Memo::new(move |_| RecordState::from_signals(recording.get(), busy.get()));
    // Raw smoothed RMS from `audio:level` (#76), in `[0, 1]`. Real speech
    // rarely drives RMS anywhere near 1.0, so `pv_amp` below applies a
    // display-only gain before handing it to `--pv-amp` — the backend value
    // stays an honest amplitude, not a UI-tuned one.
    let audio_level = RwSignal::new(0.0_f32);
    const PV_AMP_GAIN: f32 = 4.0;
    let pv_amp = move || (audio_level.get() * PV_AMP_GAIN).clamp(0.0, 1.0);
    // Seconds since capture opened, for the recording label. The backend
    // tracks its own `started_at` for `duration_ms`, but never surfaces it,
    // so this counts locally: one interval, started when `recording` goes
    // true and cleared when it goes false. Tick-counted rather than read off
    // a clock — a second's drift on a live label doesn't matter enough to
    // reach for `js_sys::Date` here (unlike the session detail timestamp
    // below, which does need it).
    let elapsed_secs = RwSignal::new(0_u32);
    Effect::new(move |previous: Option<Option<IntervalHandle>>| {
        if let Some(Some(handle)) = previous {
            handle.clear();
        }
        if !recording.get() {
            return None;
        }
        elapsed_secs.set(0);
        set_interval_with_handle(
            move || elapsed_secs.update(|secs| *secs += 1),
            Duration::from_secs(1),
        )
        .ok()
    });
    let transcript_lines = RwSignal::new(Vec::<String>::new());
    let detected_language = RwSignal::new(None::<String>);
    let error_message = RwSignal::new(None::<String>);
    let target_lang = RwSignal::new("pt".to_string());
    let settings_open = RwSignal::new(false);
    let history_open = RwSignal::new(false);
    // The sheets return focus to whichever header button opened them —
    // `../design/DESIGN.md` → *Accessibility*.
    let settings_toggle_ref = NodeRef::<leptos::html::Button>::new();
    let history_toggle_ref = NodeRef::<leptos::html::Button>::new();
    let record_button_ref = NodeRef::<leptos::html::Button>::new();
    let models = RwSignal::new(Vec::<ModelInfo>::new());
    let models_loading = RwSignal::new(false);
    let downloading_size = RwSignal::new(None::<ModelSize>);
    let translation_models = RwSignal::new(Vec::<LanguagePairInfo>::new());
    let translation_models_loading = RwSignal::new(false);
    let downloading_language = RwSignal::new(None::<String>);
    let input_devices = RwSignal::new(Vec::<InputDevice>::new());
    let devices_loading = RwSignal::new(false);
    // `None` means "let the backend pick its own default" (the microphone
    // row's "Default" option) — same convention `StartRecordingArgs`
    // already used before this picker existed.
    let selected_device_id = RwSignal::new(None::<String>);
    let sessions = RwSignal::new(Vec::<Session>::new());
    let sessions_loading = RwSignal::new(true);
    // The session detail sheet (`../design/DESIGN.md` → *Key Screens ·
    // Session*): `Some(id)` opens it on that session, from either the record
    // button (set after `stop_recording` below) or a History card
    // (`on_open_session` below). `session_detail_invoker` is whichever of
    // those actually opened it, so the sheet can return focus there on close.
    let session_detail_id = RwSignal::new(None::<String>);
    let session_detail_invoker = RwSignal::new(None::<web_sys::HtmlElement>);
    let on_open_session = Callback::new(move |(id, el): (String, web_sys::HtmlElement)| {
        session_detail_invoker.set(Some(el));
        session_detail_id.set(Some(id));
    });
    // The history list is collapsed to the newest few by default so the live
    // transcript/translation stay in view — see SESSION_PREVIEW_COUNT.
    let sessions_expanded = RwSignal::new(false);

    // The transcript is the screen, and it flows upward: newest segment at the
    // bottom, like a chat log. Pinning the scroll to the bottom whenever a
    // segment lands is what makes that true while recording. `NodeRef::get()`
    // is itself reactive, so this also fires once on mount.
    let transcript_ref = NodeRef::<leptos::html::Section>::new();
    Effect::new(move |_| {
        transcript_lines.track();
        let Some(element) = transcript_ref.get() else {
            return;
        };
        // A frame late on purpose: this effect can run before the new line is
        // patched into the DOM, and `scroll_height` only counts it afterwards.
        request_animation_frame(move || {
            element.set_scroll_top(element.scroll_height());
        });
    });

    // Purely reactive per DEC-007: listen for live transcript segments,
    // never poll a status command.
    spawn_local(async move {
        let mut events =
            match tauri_sys::event::listen::<TranscriptSegment>("transcript:segment").await {
                Ok(stream) => stream,
                Err(e) => {
                    error_message.set(Some(format!("failed to listen for transcript events: {e}")));
                    return;
                }
            };
        while let Some(event) = events.next().await {
            let segment = event.payload;
            transcript_lines.update(|lines| lines.push(segment.text));
            detected_language.set(Some(segment.language));
        }
    });

    // Live meter per #76: drives `--pv-amp` off the mic's actual level
    // instead of the fixed idle-loop breathing animation while recording.
    spawn_local(async move {
        let mut events = match tauri_sys::event::listen::<AudioLevel>("audio:level").await {
            Ok(stream) => stream,
            Err(e) => {
                error_message.set(Some(format!(
                    "failed to listen for audio level events: {e}"
                )));
                return;
            }
        };
        while let Some(event) = events.next().await {
            audio_level.set(event.payload.level);
        }
    });

    spawn_local(async move {
        let args = ListSessionsArgs {
            limit: Some(20),
            offset: Some(0),
        };
        match tauri_sys::core::invoke_result::<Vec<Session>, String>("list_sessions", args).await {
            Ok(list) => sessions.set(list),
            Err(e) => error_message.set(Some(e)),
        }
        sessions_loading.set(false);
    });

    let toggle_recording = move || {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        error_message.set(None);
        spawn_local(async move {
            if recording.get_untracked() {
                match tauri_sys::core::invoke_result::<String, String>("stop_recording", ()).await {
                    Ok(id) => {
                        recording.set(false);
                        audio_level.set(0.0);
                        // Opens the session detail sheet on the just-finished
                        // session (`../design/DESIGN.md` → *Key Screens ·
                        // Session*: "Opened after stopping..."), returning
                        // focus to the record button on close since that's
                        // what the user actually pressed to get here.
                        session_detail_invoker.set(
                            record_button_ref
                                .get_untracked()
                                .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok()),
                        );
                        session_detail_id.set(Some(id));
                        spawn_local(async move {
                            let args = ListSessionsArgs {
                                limit: Some(20),
                                offset: Some(0),
                            };
                            if let Ok(list) =
                                tauri_sys::core::invoke_result::<Vec<Session>, String>(
                                    "list_sessions",
                                    args,
                                )
                                .await
                            {
                                sessions.set(list);
                            }
                        });
                    }
                    Err(e) => error_message.set(Some(e)),
                }
            } else {
                transcript_lines.set(Vec::new());
                detected_language.set(None);
                session_detail_id.set(None);
                let args = StartRecordingArgs {
                    device_id: selected_device_id.get_untracked(),
                };
                match tauri_sys::core::invoke_result::<(), String>("start_recording", args).await {
                    Ok(()) => recording.set(true),
                    Err(e) => error_message.set(Some(e)),
                }
            }
            busy.set(false);
        });
    };

    // Local record-toggle accelerator (#125) — window-scoped, not a
    // `tauri-plugin-global-shortcut` registration, so it's only live while
    // this window is focused. Only fires on the record screen (no sheet
    // open) so it can't silently start/stop a recording behind Settings,
    // History, or a session detail sheet. `prevent_default` stops Space
    // from also scrolling the page or activating whatever control has
    // focus; `repeat()` is ignored so holding the key down doesn't
    // rapid-toggle.
    window_event_listener(leptos::ev::keydown, move |ev| {
        if ev.repeat() || ev.ctrl_key() || ev.meta_key() || ev.alt_key() || ev.shift_key() {
            return;
        }
        if settings_open.get_untracked()
            || history_open.get_untracked()
            || session_detail_id.get_untracked().is_some()
        {
            return;
        }
        if ev.code() != record_shortcut.get_untracked().code() {
            return;
        }
        ev.prevent_default();
        toggle_recording();
    });

    let toggle_settings = move |_| {
        let opening = !settings_open.get_untracked();
        settings_open.set(opening);
        if opening {
            // One panel at a time — they share the main area (see the view).
            history_open.set(false);
            models_loading.set(true);
            spawn_local(async move {
                match tauri_sys::core::invoke_result::<Vec<ModelInfo>, String>("list_models", ())
                    .await
                {
                    Ok(list) => models.set(list),
                    Err(e) => error_message.set(Some(e)),
                }
                models_loading.set(false);
            });
            translation_models_loading.set(true);
            spawn_local(async move {
                match tauri_sys::core::invoke_result::<Vec<LanguagePairInfo>, String>(
                    "list_translation_models",
                    (),
                )
                .await
                {
                    Ok(list) => translation_models.set(list),
                    Err(e) => error_message.set(Some(e)),
                }
                translation_models_loading.set(false);
            });
            devices_loading.set(true);
            spawn_local(async move {
                match tauri_sys::core::invoke_result::<Vec<InputDevice>, String>(
                    "list_input_devices",
                    (),
                )
                .await
                {
                    Ok(list) => input_devices.set(list),
                    Err(e) => error_message.set(Some(e)),
                }
                devices_loading.set(false);
            });
        }
    };

    view! {
        <div class="shell">
            // The persistent left rail (>= 900px, `../design/DESIGN.md` →
            // *Layout*) — the same `SessionList` the History sheet renders
            // below 900px, just always mounted and never modal. CSS alone
            // decides which of the two is visible at a given width; nothing
            // here reacts to viewport size.
            <aside class="history-rail" aria-label="History">
                <h2>"History"</h2>
                <SessionList
                    sessions=sessions
                    sessions_loading=sessions_loading
                    sessions_expanded=sessions_expanded
                    on_open=on_open_session
                    error_message=error_message
                />
            </aside>
        <main class="app">
            <header class="app-header">
                <div class="brand">
                    <PolyVocalMark size=24/>
                    <h1>"PolyVocal"</h1>
                </div>
                <div class="header-actions">
                    <button
                        class="history-toggle"
                        class:is-active=move || history_open.get()
                        node_ref=history_toggle_ref
                        on:click=move |_| {
                            let opening = !history_open.get_untracked();
                            history_open.set(opening);
                            if opening {
                                settings_open.set(false);
                            }
                        }
                        title="History"
                        aria-label="History"
                        aria-expanded=move || if history_open.get() { "true" } else { "false" }
                    >
                        <History/>
                    </button>
                    <button
                        class="settings-toggle"
                        class:is-active=move || settings_open.get()
                        node_ref=settings_toggle_ref
                        on:click=toggle_settings
                        title="Settings"
                        aria-label="Settings"
                        aria-expanded=move || if settings_open.get() { "true" } else { "false" }
                    >
                        <Settings/>
                    </button>
                </div>
            </header>

            <Sheet
                open=Signal::derive(move || settings_open.get())
                on_close=Callback::new(move |_| settings_open.set(false))
                title=Signal::derive(|| "Settings".to_string())
                variant="settings-sheet"
                invoker=Signal::derive(move || {
                    settings_toggle_ref.get().and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
                })
                header_extra=std::sync::Arc::new(|| ().into_any())
            >
                {move || {
                    if models_loading.get() || translation_models_loading.get() || devices_loading.get() {
                        view! { <p class="sessions-empty">"Loading…"</p> }.into_any()
                    } else {
                        view! {
                            <h2><Languages/> <span>"Languages"</span></h2>
                            // Language pairs, not files (`../../design/DESIGN.md`
                            // principle 5) — each row is "Portuguese ↔ English",
                            // never the underlying `opus-mt-*` checkpoint names
                            // `translation_models_for_language` maps it to on the
                            // backend. No "real" progress percentage exists for
                            // these multi-hundred-MB fetches (the downloader
                            // streams to disk without reporting bytes read), so
                            // — same judgment call as the Whisper list below —
                            // this shows an indeterminate "Downloading…" state
                            // rather than a percentage.
                            <ul class="model-list">
                                {translation_models.get().into_iter().map(|pair| {
                                    let language_for_click = pair.language.clone();
                                    let language_for_label = pair.language.clone();
                                    let pair_label = language_label(&pair.language);
                                    view! {
                                        <li class="model-item">
                                            <div class="model-body">
                                                <p class="model-name">
                                                    {pair_label}" ↔ English · "{pair.size_mb}" MB"
                                                </p>
                                            </div>
                                            {if pair.downloaded {
                                                view! { <span class="model-ready">"Ready"</span> }.into_any()
                                            } else {
                                                view! {
                                                    <button
                                                        class="model-download"
                                                        disabled=move || downloading_language.get().is_some()
                                                        on:click=move |_| {
                                                            let language = language_for_click.clone();
                                                            downloading_language.set(Some(language.clone()));
                                                            spawn_local(async move {
                                                                let args = DownloadTranslationModelArgs { language };
                                                                if let Err(e) = tauri_sys::core::invoke_result::<(), String>("download_translation_model", args).await {
                                                                    error_message.set(Some(e));
                                                                }
                                                                match tauri_sys::core::invoke_result::<Vec<LanguagePairInfo>, String>("list_translation_models", ()).await {
                                                                    Ok(list) => translation_models.set(list),
                                                                    Err(e) => error_message.set(Some(e)),
                                                                }
                                                                downloading_language.set(None);
                                                            });
                                                        }
                                                    >
                                                        {move || if downloading_language.get().as_deref() == Some(language_for_label.as_str()) { "Downloading…" } else { "Download" }}
                                                    </button>
                                                }.into_any()
                                            }}
                                        </li>
                                    }
                                }).collect_view()}
                            </ul>

                            <h2>"Accuracy"</h2>
                            <ul class="model-list">
                                {models.get().into_iter().filter(|m| MODEL_PICKER_SIZES.contains(&m.size)).map(|model| {
                                    let size = model.size;
                                    view! {
                                        <li class="model-item">
                                            <div class="model-body">
                                                <p class="model-name">
                                                    {size.label()}" · "{size.size_mb()}" MB"
                                                </p>
                                            </div>
                                            {if model.is_active {
                                                view! { <span class="model-active">"Active"</span> }.into_any()
                                            } else if model.downloaded {
                                                view! {
                                                    <button
                                                        class="model-activate"
                                                        disabled=move || downloading_size.get().is_some()
                                                        on:click=move |_| {
                                                            spawn_local(async move {
                                                                let args = SetActiveModelArgs { size };
                                                                if let Err(e) = tauri_sys::core::invoke_result::<(), String>("set_active_model", args).await {
                                                                    error_message.set(Some(e));
                                                                }
                                                                match tauri_sys::core::invoke_result::<Vec<ModelInfo>, String>("list_models", ()).await {
                                                                    Ok(list) => models.set(list),
                                                                    Err(e) => error_message.set(Some(e)),
                                                                }
                                                            });
                                                        }
                                                    >
                                                        "Activate"
                                                    </button>
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <button
                                                        class="model-download"
                                                        disabled=move || downloading_size.get().is_some()
                                                        on:click=move |_| {
                                                            downloading_size.set(Some(size));
                                                            spawn_local(async move {
                                                                let args = DownloadModelArgs { size };
                                                                if let Err(e) = tauri_sys::core::invoke_result::<(), String>("download_model", args).await {
                                                                    error_message.set(Some(e));
                                                                }
                                                                match tauri_sys::core::invoke_result::<Vec<ModelInfo>, String>("list_models", ()).await {
                                                                    Ok(list) => models.set(list),
                                                                    Err(e) => error_message.set(Some(e)),
                                                                }
                                                                downloading_size.set(None);
                                                            });
                                                        }
                                                    >
                                                        {move || if downloading_size.get() == Some(size) { "Downloading…" } else { "Download" }}
                                                    </button>
                                                }.into_any()
                                            }}
                                        </li>
                                    }
                                }).collect_view()}
                            </ul>

                            <div class="settings-row">
                                <label for="mic-select">"Microphone"</label>
                                <select
                                    id="mic-select"
                                    aria-label="Microphone"
                                    prop:value=move || selected_device_id.get().unwrap_or_default()
                                    on:change=move |ev| {
                                        let value = event_target_value(&ev);
                                        selected_device_id.set(if value.is_empty() { None } else { Some(value) });
                                    }
                                >
                                    <option value="">"Default"</option>
                                    {input_devices.get().into_iter().map(|device| {
                                        let label = if device.is_default {
                                            format!("{} (default)", device.name)
                                        } else {
                                            device.name
                                        };
                                        view! { <option value=device.id>{label}</option> }
                                    }).collect_view()}
                                </select>
                            </div>

                            <div class="settings-row">
                                <label for="appearance-select">"Appearance"</label>
                                <select
                                    id="appearance-select"
                                    aria-label="Appearance"
                                    prop:value=move || match theme_mode.get() {
                                        ThemeMode::Auto => "auto",
                                        ThemeMode::Light => "light",
                                        ThemeMode::Dark => "dark",
                                    }
                                    on:change=move |ev| {
                                        theme_mode.set(ThemeMode::from_storage_value(&event_target_value(&ev)));
                                    }
                                >
                                    <option value="auto">{ThemeMode::Auto.label()}</option>
                                    <option value="light">{ThemeMode::Light.label()}</option>
                                    <option value="dark">{ThemeMode::Dark.label()}</option>
                                </select>
                            </div>

                            <div class="settings-row">
                                <label for="record-shortcut-select">"Record shortcut"</label>
                                <select
                                    id="record-shortcut-select"
                                    aria-label="Record shortcut"
                                    prop:value=move || record_shortcut.get().storage_value()
                                    on:change=move |ev| {
                                        record_shortcut.set(RecordShortcutKey::from_storage_value(&event_target_value(&ev)));
                                    }
                                >
                                    <option value="space">{RecordShortcutKey::Space.label()}</option>
                                    <option value="r">{RecordShortcutKey::R.label()}</option>
                                    <option value="s">{RecordShortcutKey::S.label()}</option>
                                </select>
                            </div>
                        }.into_any()
                    }
                }}
            </Sheet>

            <Sheet
                open=Signal::derive(move || history_open.get())
                on_close=Callback::new(move |_| history_open.set(false))
                title=Signal::derive(|| "History".to_string())
                variant="history-sheet"
                invoker=Signal::derive(move || {
                    history_toggle_ref.get().and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
                })
                header_extra=std::sync::Arc::new(|| ().into_any())
            >
                <SessionList
                    sessions=sessions
                    sessions_loading=sessions_loading
                    sessions_expanded=sessions_expanded
                    on_open=on_open_session
                    error_message=error_message
                />
            </Sheet>

            // Nested above History/Settings when opened from a card there
            // (`.session-detail-sheet`'s higher z-index in styles.css) so its
            // own back control returns to History rather than skipping past
            // it to the record screen.
            <SessionDetailSheet
                session_detail_id=session_detail_id
                invoker=Signal::derive(move || session_detail_invoker.get())
                sessions=sessions
                default_target_lang=target_lang
                error_message=error_message
            />

            {move || {
                error_message.get().map(|msg| view! {
                    <p class="error" role="alert"><TriangleAlert/> <span>{msg}</span></p>
                })
            }}

            // The record screen is always mounted underneath the sheets above
            // (`../design/DESIGN.md` → *Interaction Patterns · Sheets, not
            // pages*: "the record screen is always the thing underneath, you
            // can never get lost") — the sheets cover it visually and trap
            // focus while open, rather than this replacing it.
            //
            // The transcript *is* the screen (../design/DESIGN.md → *Key
            // Screens · Record*): it takes every pixel the header and the
            // action bar leave behind, and it is the only thing that scrolls.
            // No heading — a chat log doesn't need labelling, so the
            // accessible name moves to the region itself.
            <section
                class="transcript"
                node_ref=transcript_ref
                aria-label="Transcript"
                aria-live="polite"
            >
                {move || {
                    let lines = transcript_lines.get();
                    if lines.is_empty() {
                        // One line, and nothing else. No cards, no tips.
                        view! {
                            <p class="transcript-empty">"Tap the brush and start talking."</p>
                        }.into_any()
                    } else {
                        view! {
                            <div class="transcript-lines">
                                {lines
                                    .into_iter()
                                    .map(|line| view! { <p>{line}</p> })
                                    .collect_view()}
                            </div>
                        }.into_any()
                    }
                }}
            </section>

            // Pinned to the bottom of the viewport by the app's flex column —
            // language pill, then the record button and its status line, in
            // the order DESIGN.md's mock lays them out.
            <div class="action-bar">
                <div class="language-pill">
                    <span class="language-source">
                        {move || detected_language
                            .get()
                            .map(|code| language_label(&code))
                            .unwrap_or_else(|| "Auto".to_string())}
                    </span>
                    <span class="language-arrow" aria-hidden="true">"→"</span>
                    <select
                        class="language-target"
                        aria-label="Translate into"
                        prop:value=move || target_lang.get()
                        on:change=move |ev| target_lang.set(event_target_value(&ev))
                    >
                        {TARGET_LANGUAGES
                            .iter()
                            .map(|(code, label)| view! { <option value=*code>{*label}</option> })
                            .collect_view()}
                    </select>
                </div>
                // The mark is the button — sized by CSS off the button's own
                // diameter (56%), so the 88/96px breakpoint lives entirely in
                // styles.css. The `size` prop only has to stay above 24 for
                // the full-detail variant, which is what it renders at here.
                <div class="record-control">
                    <button
                        class="record-button"
                        node_ref=record_button_ref
                        class:is-recording=move || record_state.get() == RecordState::Recording
                        class:is-processing=move || record_state.get() == RecordState::Processing
                        class:is-disabled=move || record_state.get() == RecordState::Disabled
                        // Swaps the fixed idle-loop breathing (styles.css) for
                        // the mic's real level once recording is live — see
                        // the `--pv-amp` seam comment there.
                        class:has-live-level=move || recording.get()
                        style:--pv-amp=move || pv_amp().to_string()
                        on:click=move |_| toggle_recording()
                        disabled=move || busy.get()
                        aria-pressed=move || if recording.get() { "true" } else { "false" }
                        aria-labelledby="record-label"
                    >
                        <PolyVocalMark size=54/>
                    </button>
                    // The button carries no text of its own, so this visible
                    // label *is* its accessible name (`aria-labelledby`) —
                    // never an icon alone. It also carries recording state and
                    // the timer independently of any animation, which is what
                    // keeps the reduced-motion path lossless.
                    <span class="record-label" id="record-label">
                        {move || match record_state.get() {
                            RecordState::Idle => "Tap to record".to_string(),
                            RecordState::Recording => {
                                format!("{} · Tap to stop", format_elapsed(elapsed_secs.get()))
                            }
                            RecordState::Processing => "Transcribing…".to_string(),
                            RecordState::Disabled => "Starting…".to_string(),
                        }}
                    </span>
                </div>
            </div>
        </main>
        </div>
    }
}

fn main() {
    console_error_panic_hook::set_once();
    // Applied synchronously, before mount, so a stored manual override
    // (see ThemeMode) takes effect on the first frame rather than flashing
    // the OS-default theme and then flipping.
    apply_theme_mode(stored_theme_mode());
    leptos::mount::mount_to_body(App);
}

// Plain native `#[test]`s, not `wasm-bindgen-test` — this crate has no
// `[lib]` target (only `[[bin]]`), and `cargo test --bin polyvocal-ui`
// already compiles and links cleanly against the host target despite
// depending on wasm-bindgen/web-sys/tauri-sys, since none of these
// specific functions touch a JS/DOM binding. No browser or Node.js test
// runner needed for logic this pure.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_mode_from_storage_value_recognises_light_and_dark() {
        assert!(ThemeMode::from_storage_value("light") == ThemeMode::Light);
        assert!(ThemeMode::from_storage_value("dark") == ThemeMode::Dark);
    }

    #[test]
    fn theme_mode_from_storage_value_falls_back_to_auto() {
        assert!(ThemeMode::from_storage_value("") == ThemeMode::Auto);
        assert!(ThemeMode::from_storage_value("sepia") == ThemeMode::Auto);
    }

    #[test]
    fn record_shortcut_key_from_storage_value_recognises_r_and_s() {
        assert!(RecordShortcutKey::from_storage_value("r") == RecordShortcutKey::R);
        assert!(RecordShortcutKey::from_storage_value("s") == RecordShortcutKey::S);
    }

    #[test]
    fn record_shortcut_key_from_storage_value_falls_back_to_space() {
        assert!(RecordShortcutKey::from_storage_value("") == RecordShortcutKey::Space);
        assert!(RecordShortcutKey::from_storage_value("tab") == RecordShortcutKey::Space);
    }

    #[test]
    fn truncate_preview_passes_short_transcripts_through_unchanged() {
        assert_eq!(truncate_preview("hello world", 80), "hello world");
    }

    #[test]
    fn truncate_preview_cuts_long_transcripts_with_an_ellipsis() {
        let transcript = "a".repeat(100);
        let preview = truncate_preview(&transcript, 80);
        assert_eq!(preview.chars().count(), 81); // 80 chars + the ellipsis
        assert!(preview.ends_with('…'));
        assert!(preview.starts_with(&"a".repeat(80)));
    }

    #[test]
    fn truncate_preview_counts_by_char_not_byte_on_multibyte_text() {
        // Portuguese/Spanish transcripts routinely carry multi-byte UTF-8
        // (á, ã, ñ, …) — a byte-indexed truncation would panic here on a
        // non-char-boundary split; this only proves it doesn't.
        let transcript = "á".repeat(100);
        let preview = truncate_preview(&transcript, 80);
        assert_eq!(preview.chars().count(), 81);
    }

    #[test]
    fn truncate_preview_at_exactly_the_limit_is_not_truncated() {
        let transcript = "a".repeat(80);
        assert_eq!(truncate_preview(&transcript, 80), transcript);
    }
}
