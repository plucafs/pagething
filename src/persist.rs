use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ViewState {
    pub pan_x: f32,
    pub pan_y: f32,
    pub zoom: f32,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ViewStore {
    #[serde(skip)]
    path: PathBuf,
    views: HashMap<String, ViewState>,
}

fn book_key(book: &Path) -> String {
    std::fs::canonicalize(book)
        .unwrap_or_else(|_| book.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

fn default_columns() -> usize {
    crate::canvas::grid::DEFAULT_COLUMNS
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    #[serde(default = "default_columns")]
    pub columns: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            columns: default_columns(),
        }
    }
}

#[derive(Debug)]
pub struct SettingsStore {
    path: PathBuf,
    settings: Settings,
}

impl SettingsStore {
    pub fn load() -> Self {
        let path = dirs::config_dir()
            .map(|d| d.join("pagething").join("settings.json"))
            .unwrap_or_default();
        Self::load_from(path)
    }

    pub fn load_from(path: PathBuf) -> Self {
        let settings = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { path, settings }
    }

    pub fn settings(&self) -> Settings {
        self.settings
    }

    pub fn set_columns(&mut self, columns: usize) {
        self.settings.columns = columns;
        self.save();
    }

    pub fn save(&self) {
        if self.path.as_os_str().is_empty() {
            return;
        }
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.settings) {
            let _ = std::fs::write(&self.path, json);
        }
    }
}

impl ViewStore {
    pub fn load() -> Self {
        let path = dirs::config_dir()
            .map(|d| d.join("pagething").join("views.json"))
            .unwrap_or_default();
        Self::load_from(path)
    }

    pub fn load_from(path: PathBuf) -> Self {
        let views = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { path, views }
    }

    pub fn get(&self, book: &Path) -> Option<ViewState> {
        self.views.get(&book_key(book)).copied()
    }

    pub fn set(&mut self, book: &Path, state: ViewState) {
        self.views.insert(book_key(book), state);
    }

    pub fn save(&self) {
        if self.path.as_os_str().is_empty() {
            return;
        }
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string(&self.views) {
            let _ = std::fs::write(&self.path, json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_store_roundtrips() {
        let dir = std::env::temp_dir().join(format!("pagething-test-{}", std::process::id()));
        let path = dir.join("views.json");

        let mut store = ViewStore::load_from(path.clone());
        let book = PathBuf::from("/tmp/some-book.epub");
        let state = ViewState {
            pan_x: 12.5,
            pan_y: -3.0,
            zoom: 1.75,
        };
        store.set(&book, state);
        store.save();

        let reloaded = ViewStore::load_from(path.clone());
        assert_eq!(reloaded.get(&book), Some(state));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_yields_empty_store() {
        let store = ViewStore::load_from(PathBuf::from("/nonexistent-xyz/views.json"));
        assert_eq!(store.get(Path::new("/book.epub")), None);
    }

    #[test]
    fn settings_store_roundtrips() {
        let dir = std::env::temp_dir().join(format!("pagething-settings-{}", std::process::id()));
        let path = dir.join("settings.json");

        let mut store = SettingsStore::load_from(path.clone());
        assert_eq!(store.settings().columns, 4);
        store.set_columns(6);

        let reloaded = SettingsStore::load_from(path.clone());
        assert_eq!(reloaded.settings().columns, 6);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_defaults_when_file_corrupt() {
        let dir = std::env::temp_dir().join(format!("pagething-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(&path, "not json").unwrap();

        let store = SettingsStore::load_from(path);
        assert_eq!(store.settings(), Settings::default());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
