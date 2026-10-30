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

pub const MIN_MARGIN: f32 = 16.0;
pub const MAX_MARGIN: f32 = 160.0;
pub const MIN_UI_SCALE: f32 = 0.5;
pub const MAX_UI_SCALE: f32 = 3.0;
pub const MIN_FONT_SIZE: f32 = 8.0;
pub const MAX_FONT_SIZE: f32 = 32.0;

fn default_margin() -> f32 {
    48.0
}

fn default_ui_scale() -> f32 {
    1.0
}

fn default_font_size() -> f32 {
    16.0
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemePref {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PageMode {
    #[default]
    Paper,
    DarkPages,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    #[serde(default = "default_columns")]
    pub columns: usize,
    #[serde(default = "default_margin")]
    pub margin: f32,
    #[serde(default)]
    pub theme: ThemePref,
    #[serde(default)]
    pub page_mode: PageMode,
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            columns: default_columns(),
            margin: default_margin(),
            theme: ThemePref::default(),
            page_mode: PageMode::default(),
            ui_scale: default_ui_scale(),
            font_size: default_font_size(),
        }
    }
}

impl Settings {
    fn clamp(&mut self) {
        self.columns = self.columns.clamp(1, crate::features::dialogs::MAX_COLUMNS);
        self.margin = self.margin.clamp(MIN_MARGIN, MAX_MARGIN);
        self.ui_scale = self.ui_scale.clamp(MIN_UI_SCALE, MAX_UI_SCALE);
        self.font_size = self.font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
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
        let mut settings: Settings = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        settings.clamp();
        Self { path, settings }
    }

    pub fn settings(&self) -> Settings {
        self.settings
    }

    pub fn update(&mut self, f: impl FnOnce(&mut Settings)) {
        f(&mut self.settings);
        self.settings.clamp();
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
        assert_eq!(store.settings(), Settings::default());
        assert_eq!(store.settings().font_size, 16.0);
        store.update(|s| {
            s.columns = 6;
            s.margin = 72.0;
            s.theme = ThemePref::Dark;
            s.page_mode = PageMode::DarkPages;
            s.ui_scale = 1.5;
            s.font_size = 20.0;
        });

        let reloaded = SettingsStore::load_from(path.clone());
        let s = reloaded.settings();
        assert_eq!(s.columns, 6);
        assert_eq!(s.margin, 72.0);
        assert_eq!(s.theme, ThemePref::Dark);
        assert_eq!(s.page_mode, PageMode::DarkPages);
        assert_eq!(s.ui_scale, 1.5);
        assert_eq!(s.font_size, 20.0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_settings_json_loads_with_defaults() {
        let dir = std::env::temp_dir().join(format!("pagething-legacy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(&path, r#"{"columns":6}"#).unwrap();

        let s = SettingsStore::load_from(path).settings();
        assert_eq!(
            s,
            Settings {
                columns: 6,
                margin: 48.0,
                ..Settings::default()
            }
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_clamp_out_of_range_values() {
        let dir = std::env::temp_dir().join(format!("pagething-clamp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        std::fs::write(
            &path,
            r#"{"columns":0,"margin":999.0,"ui_scale":99.0,"font_size":99.0}"#,
        )
        .unwrap();

        let store = SettingsStore::load_from(path);
        let s = store.settings();
        assert_eq!(s.columns, 1);
        assert_eq!(s.margin, MAX_MARGIN);
        assert_eq!(s.ui_scale, MAX_UI_SCALE);
        assert_eq!(s.font_size, MAX_FONT_SIZE);
        assert_eq!(s.theme, ThemePref::System);
        assert_eq!(s.page_mode, PageMode::Paper);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn theme_and_page_mode_serialize_snake_case() {
        let json = serde_json::to_string(&Settings {
            theme: ThemePref::Light,
            page_mode: PageMode::DarkPages,
            ..Settings::default()
        })
        .unwrap();
        assert!(json.contains("\"theme\":\"light\""));
        assert!(json.contains("\"page_mode\":\"dark_pages\""));
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
