use crate::commands::storage::{delete_session, Session};
use crate::format::{truncate_preview, SESSION_PREVIEW_CHAR_LIMIT, SESSION_PREVIEW_COUNT};
use crate::icons::Trash2;
use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsCast;

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
pub fn SessionList(
    sessions: RwSignal<Vec<Session>>,
    sessions_loading: RwSignal<bool>,
    sessions_expanded: RwSignal<bool>,
    on_open: Callback<(String, web_sys::HtmlElement)>,
    push_toast: Callback<String>,
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
                                                    if let Err(e) = delete_session(&id).await {
                                                        push_toast.run(e);
                                                    } else {
                                                        sessions.update(|list| list.retain(|s| s.id != id));
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
