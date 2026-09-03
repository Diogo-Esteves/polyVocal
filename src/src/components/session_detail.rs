use crate::commands::storage::{
    delete_session, export_session_srt, export_session_txt, get_session, Session,
};
use crate::commands::translation::translate_text;
use crate::format::{format_duration_label, format_session_datetime, TARGET_LANGUAGES};
use crate::icons::MoreHorizontal;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::time::Duration;

use super::sheet::Sheet;

/// Which text the session detail sheet's `[ Original | English ⌄ ]` toggle
/// (`../design/DESIGN.md` → *Key Screens · Session*) is currently showing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SessionView {
    Original,
    Translated,
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
pub fn SessionDetailSheet(
    session_detail_id: RwSignal<Option<String>>,
    invoker: Signal<Option<web_sys::HtmlElement>>,
    sessions: RwSignal<Vec<Session>>,
    default_target_lang: RwSignal<String>,
    push_toast: Callback<String>,
) -> impl IntoView {
    let detail = RwSignal::new(None::<Session>);
    let loading = RwSignal::new(false);
    let view_mode = RwSignal::new(SessionView::Original);
    let target_lang = RwSignal::new(default_target_lang.get_untracked());
    let translating = RwSignal::new(false);
    let menu_open = RwSignal::new(false);
    let pending_delete = RwSignal::new(false);
    let copied = RwSignal::new(false);

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
                copied.set(false);
                target_lang.set(default_target_lang.get_untracked());
                loading.set(true);
                spawn_local(async move {
                    match get_session(&id).await {
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
                        Ok(None) => push_toast.run("Session not found.".to_string()),
                        Err(e) => push_toast.run(e),
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
        spawn_local(async move {
            match translate_text(&id, &lang).await {
                Ok(text) => {
                    detail.update(|maybe| {
                        if let Some(session) = maybe {
                            session.translation = Some(text);
                            session.target_lang = Some(lang);
                        }
                    });
                    view_mode.set(SessionView::Translated);
                }
                Err(e) => push_toast.run(e),
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
            if let Err(e) = export_session_txt(&id).await {
                push_toast.run(e);
            }
        });
    };

    let export_srt = move |_| {
        menu_open.set(false);
        let Some(id) = session_detail_id.get_untracked() else {
            return;
        };
        spawn_local(async move {
            if let Err(e) = export_session_srt(&id).await {
                push_toast.run(e);
            }
        });
    };

    // Copies whichever text the toggle is currently showing — original or
    // translated — since that's what the user is looking at. Leaves the menu
    // open through the "Copied!" swap (closing it immediately, as the other
    // menu actions do, would hide that feedback before it's ever seen) and
    // closes it itself once the swap times out.
    let copy_now = move |_| {
        let Some(session) = detail.get_untracked() else {
            return;
        };
        let text = match view_mode.get_untracked() {
            SessionView::Original => session.transcript.clone(),
            SessionView::Translated => session.translation.clone().unwrap_or_default(),
        };
        spawn_local(async move {
            let promise = window().navigator().clipboard().write_text(&text);
            match wasm_bindgen_futures::JsFuture::from(promise).await {
                Ok(_) => {
                    copied.set(true);
                    set_timeout(
                        move || {
                            copied.set(false);
                            menu_open.set(false);
                        },
                        Duration::from_millis(1500),
                    );
                }
                Err(_) => {
                    menu_open.set(false);
                    push_toast.run("Couldn't copy to clipboard.".to_string());
                }
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
                if let Err(e) = delete_session(&id).await {
                    push_toast.run(e);
                } else {
                    sessions.update(|list| list.retain(|s| s.id != id));
                    session_detail_id.set(None);
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
                        <button class="session-menu-item" on:click=copy_now>
                            {move || if copied.get() { "Copied!" } else { "Copy text" }}
                        </button>
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
