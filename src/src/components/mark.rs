use leptos::prelude::*;

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
pub fn PolyVocalMark(#[prop(default = 24)] size: u32) -> impl IntoView {
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
