use std::time::Duration;

use futures::StreamExt;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

mod commands;
mod components;
mod format;
mod icons;
mod shortcuts;
mod theme;

use commands::audio::{
    list_input_devices, start_recording, stop_recording, AudioLevel, InputDevice, TranscriptSegment,
};
use commands::config::{get_config, set_config, AppConfig};
use commands::models::{
    download_model, list_models, set_active_model, ModelInfo, ModelSize, MODEL_PICKER_SIZES,
};
use commands::storage::{list_sessions, Session};
use commands::translation::{
    download_translation_model, list_translation_models, LanguagePairInfo,
};
use components::mark::PolyVocalMark;
use components::session_detail::SessionDetailSheet;
use components::session_list::SessionList;
use components::sheet::Sheet;
use format::{format_elapsed, language_label, TARGET_LANGUAGES};
use icons::{History, Languages, Settings, TriangleAlert};
use shortcuts::{apply_record_shortcut, stored_record_shortcut, RecordShortcutKey};
use theme::{apply_theme_mode, stored_theme_mode, ThemeMode};

/// A dismissible toast notification in the stacking queue.
#[derive(Clone)]
struct Toast {
    id: u32,
    message: String,
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
    let toasts = RwSignal::new(Vec::<Toast>::new());
    let next_toast_id = RwSignal::new(0u32);
    let push_toast = Callback::new(move |message: String| {
        let id = next_toast_id.get_untracked();
        next_toast_id.set(id + 1);
        toasts.update(|t| t.push(Toast { id, message }));
        set_timeout(
            move || {
                toasts.update(|t| t.retain(|toast| toast.id != id));
            },
            Duration::from_secs(6),
        );
    });
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

    let persist_config = move || {
        let cfg = AppConfig {
            input_device: selected_device_id.get_untracked(),
            target_lang: Some(target_lang.get_untracked()),
        };
        spawn_local(async move {
            if let Err(e) = set_config(cfg).await {
                push_toast.run(e);
            }
        });
    };

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
                    push_toast.run(format!("failed to listen for transcript events: {e}"));
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
                push_toast.run(format!("failed to listen for audio level events: {e}"));
                return;
            }
        };
        while let Some(event) = events.next().await {
            audio_level.set(event.payload.level);
        }
    });

    spawn_local(async move {
        match list_sessions().await {
            Ok(list) => sessions.set(list),
            Err(e) => push_toast.run(e),
        }
        sessions_loading.set(false);
    });

    spawn_local(async move {
        if let Ok(cfg) = get_config().await {
            selected_device_id.set(cfg.input_device);
            if let Some(lang) = cfg.target_lang {
                target_lang.set(lang);
            }
        }
    });

    let toggle_recording = move || {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        spawn_local(async move {
            if recording.get_untracked() {
                match stop_recording().await {
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
                            if let Ok(list) = list_sessions().await {
                                sessions.set(list);
                            }
                        });
                    }
                    Err(e) => push_toast.run(e),
                }
            } else {
                transcript_lines.set(Vec::new());
                detected_language.set(None);
                session_detail_id.set(None);
                match start_recording(selected_device_id.get_untracked()).await {
                    Ok(()) => recording.set(true),
                    Err(e) => push_toast.run(e),
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
                match list_models().await {
                    Ok(list) => models.set(list),
                    Err(e) => push_toast.run(e),
                }
                models_loading.set(false);
            });
            translation_models_loading.set(true);
            spawn_local(async move {
                match list_translation_models().await {
                    Ok(list) => translation_models.set(list),
                    Err(e) => push_toast.run(e),
                }
                translation_models_loading.set(false);
            });
            devices_loading.set(true);
            spawn_local(async move {
                match list_input_devices().await {
                    Ok(list) => input_devices.set(list),
                    Err(e) => push_toast.run(e),
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
                    push_toast=push_toast
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
                                                                if let Err(e) = download_translation_model(&language).await {
                                                                    push_toast.run(e);
                                                                }
                                                                match list_translation_models().await {
                                                                    Ok(list) => translation_models.set(list),
                                                                    Err(e) => push_toast.run(e),
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
                                                                if let Err(e) = set_active_model(size).await {
                                                                    push_toast.run(e);
                                                                }
                                                                match list_models().await {
                                                                    Ok(list) => models.set(list),
                                                                    Err(e) => push_toast.run(e),
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
                                                                if let Err(e) = download_model(size).await {
                                                                    push_toast.run(e);
                                                                }
                                                                match list_models().await {
                                                                    Ok(list) => models.set(list),
                                                                    Err(e) => push_toast.run(e),
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
                                        persist_config();
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
                    push_toast=push_toast
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
                push_toast=push_toast
            />

            <div class="toast-stack">
                <For
                    each=move || toasts.get()
                    key=|toast| toast.id
                    children=move |toast| {
                        let id = toast.id;
                        view! {
                            <p class="toast" role="alert">
                                <TriangleAlert/>
                                <span>{toast.message}</span>
                                <button
                                    type="button"
                                    class="toast-dismiss"
                                    aria-label="Dismiss"
                                    on:click=move |_| toasts.update(|t| t.retain(|toast| toast.id != id))
                                >
                                    "×"
                                </button>
                            </p>
                        }
                    }
                />
            </div>

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
                        on:change=move |ev| {
                            target_lang.set(event_target_value(&ev));
                            persist_config();
                        }
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
    use crate::format::truncate_preview;
    use crate::shortcuts::RecordShortcutKey;
    use crate::theme::ThemeMode;

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
