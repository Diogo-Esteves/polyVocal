use leptos::prelude::*;

/// Key the record-toggle keyboard shortcut is persisted under in
/// `localStorage`.
const RECORD_SHORTCUT_STORAGE_KEY: &str = "polyvocal-record-shortcut";

/// The key that toggles recording (see `App`'s `window_event_listener`).
/// A local accelerator only — scoped to the app window and only live while
/// the record screen is showing, not a `tauri-plugin-global-shortcut`
/// registration (see #125).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RecordShortcutKey {
    Space,
    R,
    S,
}

impl RecordShortcutKey {
    /// The `KeyboardEvent.code` value to match against — layout-independent,
    /// unlike `.key()`.
    pub fn code(self) -> &'static str {
        match self {
            RecordShortcutKey::Space => "Space",
            RecordShortcutKey::R => "KeyR",
            RecordShortcutKey::S => "KeyS",
        }
    }

    pub fn storage_value(self) -> &'static str {
        match self {
            RecordShortcutKey::Space => "space",
            RecordShortcutKey::R => "r",
            RecordShortcutKey::S => "s",
        }
    }

    pub fn from_storage_value(value: &str) -> Self {
        match value {
            "r" => RecordShortcutKey::R,
            "s" => RecordShortcutKey::S,
            _ => RecordShortcutKey::Space,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RecordShortcutKey::Space => "Space",
            RecordShortcutKey::R => "R",
            RecordShortcutKey::S => "S",
        }
    }
}

/// Reads the persisted record-shortcut key. Falls back to `Space`.
pub fn stored_record_shortcut() -> RecordShortcutKey {
    window()
        .local_storage()
        .ok()
        .flatten()
        .and_then(|storage| storage.get_item(RECORD_SHORTCUT_STORAGE_KEY).ok().flatten())
        .map(|value| RecordShortcutKey::from_storage_value(&value))
        .unwrap_or(RecordShortcutKey::Space)
}

/// Persists the record-toggle shortcut key choice.
pub fn apply_record_shortcut(key: RecordShortcutKey) {
    if let Ok(Some(storage)) = window().local_storage() {
        let _ = storage.set_item(RECORD_SHORTCUT_STORAGE_KEY, key.storage_value());
    }
}
