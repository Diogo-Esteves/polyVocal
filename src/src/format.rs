/// `M:SS` for the recording label. Deliberately not zero-padded on the
/// minutes — the label reads "0:05 · Tap to stop", not "00:05".
pub fn format_elapsed(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// "Aug 14, 19:11" for the session detail header (`../design/DESIGN.md` →
/// *Key Screens · Session*). `created_at` is an RFC 3339 UTC string (backend
/// uses `chrono::Utc::now().to_rfc3339()`); `js_sys::Date` parses that
/// directly and its plain (non-UTC) getters convert to the OS local time
/// zone for free — a timestamp is more useful read in the time the user
/// actually experienced it than in UTC.
pub fn format_session_datetime(created_at: &str) -> String {
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
pub fn format_duration_label(duration_ms: i64) -> String {
    let secs = (duration_ms.max(0) / 1000) as u32;
    if secs < 60 {
        format!("{secs}s")
    } else {
        format_elapsed(secs)
    }
}

/// How many of the (newest-first) sessions the history list shows before the
/// user asks for the rest. Keeps "Recent Sessions" from pushing the live
/// transcript and translation off the bottom of the screen.
pub const SESSION_PREVIEW_COUNT: usize = 3;

/// Character budget for a session card's transcript preview before it's
/// truncated with a trailing "…".
pub const SESSION_PREVIEW_CHAR_LIMIT: usize = 80;

/// Truncates `transcript` to at most `max_chars` characters, appending "…"
/// when it was actually cut short. Counts by `char`, not byte, so this is
/// safe on multi-byte UTF-8 transcripts (translated/non-English sessions).
pub fn truncate_preview(transcript: &str, max_chars: usize) -> String {
    if transcript.chars().count() > max_chars {
        let truncated: String = transcript.chars().take(max_chars).collect();
        format!("{truncated}…")
    } else {
        transcript.to_string()
    }
}

/// MVP language pairs, matching `translation::SUPPORTED_LANGUAGES` in the backend.
pub const TARGET_LANGUAGES: [(&str, &str); 3] =
    [("en", "English"), ("pt", "Portuguese"), ("es", "Spanish")];

/// Human name for an ISO code, for the action bar's language pill —
/// "Languages, not files" (`../design/DESIGN.md` → *Design Principles*).
/// Whisper detects far more languages than the three we can translate
/// between, so anything outside `TARGET_LANGUAGES` falls back to the raw
/// code rather than being hidden.
pub fn language_label(code: &str) -> String {
    TARGET_LANGUAGES
        .iter()
        .find(|(candidate, _)| *candidate == code)
        .map(|(_, label)| (*label).to_string())
        .unwrap_or_else(|| code.to_uppercase())
}
