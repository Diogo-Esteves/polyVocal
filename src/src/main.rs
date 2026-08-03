use futures::StreamExt;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

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

#[component]
fn App() -> impl IntoView {
    let recording = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let session_id = RwSignal::new(None::<String>);
    let transcript_lines = RwSignal::new(Vec::<String>::new());
    let detected_language = RwSignal::new(None::<String>);
    let error_message = RwSignal::new(None::<String>);
    let target_lang = RwSignal::new("pt".to_string());
    let translated_text = RwSignal::new(None::<String>);
    let translating = RwSignal::new(false);

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

    view! {
        <main class="app">
            <h1>"PolyVocal"</h1>

            <section class="controls">
                <button on:click=toggle_recording disabled=move || busy.get()>
                    {move || if recording.get() { "Stop" } else { "Record" }}
                </button>
                <span class="language">
                    "Detected language: "
                    {move || detected_language.get().unwrap_or_else(|| "—".to_string())}
                </span>
            </section>

            {move || {
                error_message.get().map(|msg| view! { <p class="error">{msg}</p> })
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
                <h2>"Translate"</h2>
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
                    "Translate"
                </button>
                <p class="translated">{move || translated_text.get().unwrap_or_default()}</p>
            </section>
        </main>
    }
}

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}
