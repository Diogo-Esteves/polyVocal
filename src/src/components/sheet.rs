use crate::icons::ArrowLeft;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

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
pub fn focusable_elements(container: &web_sys::HtmlDivElement) -> Vec<web_sys::HtmlElement> {
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
pub fn Sheet(
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
