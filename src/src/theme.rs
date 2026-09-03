use leptos::prelude::*;

/// Key the manual theme override is persisted under in `localStorage`.
const THEME_STORAGE_KEY: &str = "polyvocal-theme";

/// Theme selection. `Auto` (the default) defers entirely to the OS via the
/// `prefers-color-scheme` CSS media query — no `data-theme` attribute is
/// set. `Light`/`Dark` are an explicit manual override, applied via
/// `data-theme` on `<html>` and persisted so it survives a reload.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
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

    pub fn from_storage_value(value: &str) -> Self {
        match value {
            "light" => ThemeMode::Light,
            "dark" => ThemeMode::Dark,
            _ => ThemeMode::Auto,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Auto => "Match system",
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
        }
    }
}

/// Reads the persisted manual override, if any. Falls back to `Auto`
/// (letting `prefers-color-scheme` decide) when nothing is stored yet.
pub fn stored_theme_mode() -> ThemeMode {
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
pub fn apply_theme_mode(mode: ThemeMode) {
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
