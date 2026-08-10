use futures::StreamExt;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

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

    fn next(self) -> Self {
        match self {
            ThemeMode::Auto => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Auto,
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

/// Minimal inline icons, adapted from Lucide (ISC license — ../design/DESIGN.md).
/// Stroke-only, `currentColor`, and `aria-hidden` since every icon here is
/// paired with a text label — icons never carry meaning on their own.
mod icons {
    use leptos::prelude::*;

    #[component]
    pub fn Mic() -> impl IntoView {
        view! {
            <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M12 19v3"/>
                <path d="M19 10v2a7 7 0 0 1-14 0v-2"/>
                <rect x="9" y="2" width="6" height="13" rx="3"/>
            </svg>
        }
    }

    #[component]
    pub fn StopSquare() -> impl IntoView {
        view! {
            <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <rect width="18" height="18" x="3" y="3" rx="2"/>
            </svg>
        }
    }

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
    pub fn Sun() -> impl IntoView {
        view! {
            <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <circle cx="12" cy="12" r="4"/>
                <path d="M12 2v2"/>
                <path d="M12 20v2"/>
                <path d="m4.93 4.93 1.41 1.41"/>
                <path d="m17.66 17.66 1.41 1.41"/>
                <path d="M2 12h2"/>
                <path d="M20 12h2"/>
                <path d="m6.34 17.66-1.41 1.41"/>
                <path d="m19.07 4.93-1.41 1.41"/>
            </svg>
        }
    }

    #[component]
    pub fn Moon() -> impl IntoView {
        view! {
            <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M20.985 12.486a9 9 0 1 1-9.473-9.472c.405-.022.617.46.402.803a6 6 0 0 0 8.268 8.268c.344-.215.825-.004.803.401"/>
            </svg>
        }
    }

    #[component]
    pub fn SunMoon() -> impl IntoView {
        view! {
            <svg class="icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M12 2v2"/>
                <path d="M14.837 16.385a6 6 0 1 1-7.223-7.222c.624-.147.97.66.715 1.248a4 4 0 0 0 5.26 5.259c.589-.255 1.396.09 1.248.715"/>
                <path d="M16 12a4 4 0 0 0-4-4"/>
                <path d="m19 5-1.256 1.256"/>
                <path d="M20 12h2"/>
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
use icons::{Languages, Mic, Moon, Settings, StopSquare, Sun, SunMoon, TriangleAlert};

/// Mirrors the `transcript:segment` event payload emitted by the Rust
/// backend (DEC-007) — only the fields this screen renders are declared;
/// serde ignores the rest.
#[derive(Deserialize, Clone)]
struct TranscriptSegment {
    text: String,
    language: String,
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

/// MVP language pairs, matching `translation::SUPPORTED_LANGUAGES` in the backend.
const TARGET_LANGUAGES: [(&str, &str); 3] =
    [("en", "English"), ("pt", "Portuguese"), ("es", "Spanish")];

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
    fn label(self) -> &'static str {
        match self {
            ModelSize::Tiny => "Tiny",
            ModelSize::Base => "Base",
            ModelSize::Small => "Small",
            ModelSize::Medium => "Medium",
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

#[component]
fn App() -> impl IntoView {
    let theme_mode = RwSignal::new(stored_theme_mode());
    Effect::new(move |_| apply_theme_mode(theme_mode.get()));
    let cycle_theme = move |_| theme_mode.update(|mode| *mode = mode.next());

    let recording = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let session_id = RwSignal::new(None::<String>);
    let transcript_lines = RwSignal::new(Vec::<String>::new());
    let detected_language = RwSignal::new(None::<String>);
    let error_message = RwSignal::new(None::<String>);
    let target_lang = RwSignal::new("pt".to_string());
    let translated_text = RwSignal::new(None::<String>);
    let translating = RwSignal::new(false);
    let settings_open = RwSignal::new(false);
    let models = RwSignal::new(Vec::<ModelInfo>::new());
    let models_loading = RwSignal::new(false);
    let downloading_size = RwSignal::new(None::<ModelSize>);

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

    let toggle_recording = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        error_message.set(None);
        spawn_local(async move {
            if recording.get_untracked() {
                match tauri_sys::core::invoke_result::<String, String>("stop_recording", ()).await {
                    Ok(id) => {
                        session_id.set(Some(id));
                        recording.set(false);
                    }
                    Err(e) => error_message.set(Some(e)),
                }
            } else {
                transcript_lines.set(Vec::new());
                detected_language.set(None);
                translated_text.set(None);
                session_id.set(None);
                let args = StartRecordingArgs { device_id: None };
                match tauri_sys::core::invoke_result::<(), String>("start_recording", args).await {
                    Ok(()) => recording.set(true),
                    Err(e) => error_message.set(Some(e)),
                }
            }
            busy.set(false);
        });
    };

    let do_translate = move |_| {
        let Some(id) = session_id.get_untracked() else {
            return;
        };
        translating.set(true);
        error_message.set(None);
        spawn_local(async move {
            let lang = target_lang.get_untracked();
            let args = TranslateArgs {
                session_id: &id,
                target_lang: &lang,
            };
            match tauri_sys::core::invoke_result::<String, String>("translate_text", args).await {
                Ok(text) => translated_text.set(Some(text)),
                Err(e) => error_message.set(Some(e)),
            }
            translating.set(false);
        });
    };

    let toggle_settings = move |_| {
        let opening = !settings_open.get_untracked();
        settings_open.set(opening);
        if opening {
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
        }
    };

    view! {
        <main class="app">
            <header class="app-header">
                <h1>"PolyVocal"</h1>
                <div class="header-actions">
                    <button
                        class="settings-toggle"
                        class:is-active=move || settings_open.get()
                        on:click=toggle_settings
                        title="Settings"
                        aria-label="Settings"
                    >
                        <Settings/>
                    </button>
                    <button
                        class="theme-toggle"
                        on:click=cycle_theme
                        title=move || format!("Theme: {} (click to change)", theme_mode.get().label())
                        aria-label=move || format!("Theme: {}. Click to change.", theme_mode.get().label())
                    >
                        {move || match theme_mode.get() {
                            ThemeMode::Auto => view! { <SunMoon/> }.into_any(),
                            ThemeMode::Light => view! { <Sun/> }.into_any(),
                            ThemeMode::Dark => view! { <Moon/> }.into_any(),
                        }}
                    </button>
                </div>
            </header>

            {move || settings_open.get().then(|| view! {
                <section class="settings">
                    <h2>"Settings"</h2>
                    {move || {
                        if models_loading.get() {
                            view! { <p class="sessions-empty">"Loading…"</p> }.into_any()
                        } else {
                            view! {
                                <ul class="model-list">
                                    {models.get().into_iter().map(|model| {
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
                            }.into_any()
                        }
                    }}
                </section>
            })}

            <section class="controls">
                <button
                    class="record-toggle"
                    class:is-recording=move || recording.get()
                    on:click=toggle_recording
                    disabled=move || busy.get()
                >
                    {move || if recording.get() {
                        view! { <StopSquare/> <span>"Stop"</span> }.into_any()
                    } else {
                        view! { <Mic/> <span>"Record"</span> }.into_any()
                    }}
                </button>
                {move || recording.get().then(|| view! {
                    <span class="recording-indicator">
                        <span class="recording-dot"></span>
                        "Recording"
                    </span>
                })}
                <span class="language">
                    "Detected language: "
                    {move || detected_language.get().unwrap_or_else(|| "—".to_string())}
                </span>
            </section>

            {move || {
                error_message.get().map(|msg| view! {
                    <p class="error"><TriangleAlert/> <span>{msg}</span></p>
                })
            }}

            <section class="transcript">
                <h2>"Transcript"</h2>
                <div class="transcript-lines">
                    {move || {
                        transcript_lines
                            .get()
                            .into_iter()
                            .map(|line| view! { <p>{line}</p> })
                            .collect_view()
                    }}
                </div>
            </section>

            <section class="translate">
                <h2><Languages/> <span>"Translate"</span></h2>
                <div class="controls">
                    <select
                        prop:value=move || target_lang.get()
                        on:change=move |ev| target_lang.set(event_target_value(&ev))
                    >
                        {TARGET_LANGUAGES
                            .iter()
                            .map(|(code, label)| view! { <option value=*code>{*label}</option> })
                            .collect_view()}
                    </select>
                    <button
                        on:click=do_translate
                        disabled=move || session_id.get().is_none() || translating.get()
                    >
                        {move || if translating.get() { "Translating…" } else { "Translate" }}
                    </button>
                </div>
                {move || translating.get().then(|| view! {
                    <p class="translate-status">"Running locally — usually a few seconds, longer the first time a language pair's model needs downloading."</p>
                })}
                <p class="translated">{move || translated_text.get().unwrap_or_default()}</p>
            </section>
        </main>
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
