use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ViewState {
    pub pan_x: f32,
    pub pan_y: f32,
    pub zoom: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct SavedPoint {
    pub page: usize,
    pub view: ViewState,
    #[serde(default)]
    pub saved_at: u64,
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
pub const MIN_PAGE_GAP: f32 = 0.0;
pub const MAX_PAGE_GAP: f32 = 200.0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PageRatio {
    Square,
    ThreeFour,
    A5,
    #[default]
    TwoThree,
    FiveEight,
}

impl PageRatio {
    pub fn height_over_width(self) -> f32 {
        match self {
            Self::Square => 1.0,
            Self::ThreeFour => 4.0 / 3.0,
            Self::A5 => 2.0_f32.sqrt(),
            Self::TwoThree => 1.5,
            Self::FiveEight => 1.6,
        }
    }
}

fn default_margin() -> f32 {
    96.0
}

fn default_ui_scale() -> f32 {
    1.0
}

fn default_font_size() -> f32 {
    16.0
}

fn default_page_gap() -> f32 {
    crate::canvas::grid::GAP
}

fn default_page_bg() -> [u8; 3] {
    [247, 243, 234]
}

fn default_page_font() -> [u8; 3] {
    [43, 38, 33]
}

fn default_app_bg() -> [u8; 3] {
    [38, 38, 38]
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
    Sepia,
    Custom,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PageTexture {
    Plain,
    #[default]
    FineGrain,
    PaperFibers,
    LinenWeave,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AppBgMode {
    #[default]
    Dark,
    Light,
    Custom,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Justify {
    #[default]
    Justified,
    Left,
    Center,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum BookFont {
    #[default]
    Serif,
    Sans,
    Monospace,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LineSpacing {
    Compact,
    #[default]
    Normal,
    Spacious,
}

impl LineSpacing {
    pub fn multiplier(self) -> f32 {
        match self {
            Self::Compact => 1.2,
            Self::Normal => 1.4,
            Self::Spacious => 1.7,
        }
    }
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
    #[serde(default = "default_page_bg")]
    pub page_bg: [u8; 3],
    #[serde(default = "default_page_font")]
    pub page_font: [u8; 3],
    #[serde(default)]
    pub page_texture: PageTexture,
    #[serde(default)]
    pub app_bg_mode: AppBgMode,
    #[serde(default = "default_app_bg")]
    pub app_bg: [u8; 3],
    #[serde(default)]
    pub justify: Justify,
    #[serde(default)]
    pub book_font: BookFont,
    #[serde(default)]
    pub line_spacing: LineSpacing,
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
    #[serde(default = "default_font_size")]
    pub font_size: f32,
    #[serde(default = "default_page_gap")]
    pub page_gap: f32,
    #[serde(default)]
    pub page_ratio: PageRatio,
    #[serde(default = "default_toolbar")]
    pub toolbar: bool,
}

fn default_toolbar() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            columns: default_columns(),
            margin: default_margin(),
            theme: ThemePref::default(),
            page_mode: PageMode::default(),
            page_bg: default_page_bg(),
            page_font: default_page_font(),
            page_texture: PageTexture::default(),
            app_bg_mode: AppBgMode::default(),
            app_bg: default_app_bg(),
            justify: Justify::default(),
            book_font: BookFont::default(),
            line_spacing: LineSpacing::default(),
            ui_scale: default_ui_scale(),
            font_size: default_font_size(),
            page_gap: default_page_gap(),
            page_ratio: PageRatio::default(),
            toolbar: default_toolbar(),
        }
    }
}

impl Settings {
    fn clamp(&mut self) {
        self.columns = self.columns.clamp(1, crate::features::dialogs::MAX_COLUMNS);
        self.margin = self.margin.clamp(MIN_MARGIN, MAX_MARGIN);
        self.ui_scale = self.ui_scale.clamp(MIN_UI_SCALE, MAX_UI_SCALE);
        self.font_size = self.font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        self.page_gap = self.page_gap.clamp(MIN_PAGE_GAP, MAX_PAGE_GAP);
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

/// User-assigned display titles for books, keyed by canonical path.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TitleStore {
    #[serde(skip)]
    path: PathBuf,
    titles: HashMap<String, String>,
}

impl TitleStore {
    pub fn load() -> Self {
        let path = dirs::config_dir()
            .map(|d| d.join("pagething").join("titles.json"))
            .unwrap_or_default();
        Self::load_from(path)
    }

    pub fn load_from(path: PathBuf) -> Self {
        let titles = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { path, titles }
    }

    pub fn get(&self, book: &Path) -> Option<&str> {
        self.titles.get(&book_key(book)).map(String::as_str)
    }

    pub fn set(&mut self, book: &Path, title: String) {
        self.titles.insert(book_key(book), title);
    }

    pub fn remove(&mut self, book: &Path) {
        self.titles.remove(&book_key(book));
    }

    pub fn save(&self) {
        if self.path.as_os_str().is_empty() {
            return;
        }
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.titles) {
            let _ = std::fs::write(&self.path, json);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    pub text: String,
    #[serde(default)]
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Postit {
    pub page: usize,
    pub text: String,
    /// Plain-text range of the highlight created together with this note (via `H`).
    #[serde(default)]
    pub highlight: Option<(usize, usize)>,
    #[serde(default)]
    pub anchor: Option<TextAnchor>,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub replies: Vec<Reply>,
    /// Collapsed to a small square on the canvas (default: expanded).
    #[serde(default)]
    pub collapsed: bool,
}

impl Postit {
    pub fn new(page: usize, text: String, highlight: Option<(usize, usize)>) -> Self {
        let now = unix_now();
        Self {
            page,
            text,
            highlight,
            anchor: None,
            created_at: now,
            updated_at: now,
            replies: Vec::new(),
            collapsed: false,
        }
    }
}

/// A persistent text highlight: char range in the page's `plain_text`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Highlight {
    /// 1-based page number (same as [`Postit::page`]).
    pub page: usize,
    pub start: usize,
    pub end: usize,
    #[serde(default)]
    pub anchor: Option<TextAnchor>,
    #[serde(default)]
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextAnchor {
    pub quote: String,
    #[serde(default)]
    pub before: String,
    #[serde(default)]
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Bookmark {
    pub page: usize,
    #[serde(default)]
    pub anchor: Option<TextAnchor>,
}

impl<'de> Deserialize<'de> for Bookmark {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StoredBookmark {
            Page(usize),
            Anchored {
                page: usize,
                #[serde(default)]
                anchor: Option<TextAnchor>,
            },
        }

        Ok(match StoredBookmark::deserialize(deserializer)? {
            StoredBookmark::Page(page) => Self { page, anchor: None },
            StoredBookmark::Anchored { page, anchor } => Self { page, anchor },
        })
    }
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Format a unix timestamp as `d Mon HH:MM` (UTC).
pub fn fmt_ts(secs: u64) -> String {
    if secs == 0 {
        return "—".to_owned();
    }
    let days = secs / 86_400;
    let secs_of_day = secs % 86_400;
    let (hour, minute) = (secs_of_day / 3_600, (secs_of_day % 3_600) / 60);
    // Civil-from-days (Howard Hinnant)
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y0 = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y0 + 1 } else { y0 };
    let _ = year;
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mon = MONTHS[(month - 1) as usize];
    format!("{day} {mon} {hour:02}:{minute:02}")
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BookStore<T> {
    #[serde(skip)]
    path: PathBuf,
    data: HashMap<String, Vec<T>>,
}

impl<T> Default for BookStore<T> {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            data: HashMap::new(),
        }
    }
}

impl<T: Clone + Serialize + DeserializeOwned> BookStore<T> {
    pub fn load(filename: &str) -> Self {
        let path = dirs::config_dir()
            .map(|d| d.join("pagething").join(filename))
            .unwrap_or_default();
        Self::load_from(path)
    }

    pub fn load_from(path: PathBuf) -> Self {
        let data = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self { path, data }
    }

    pub fn get(&self, book: &Path) -> &[T] {
        self.data
            .get(&book_key(book))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn set(&mut self, book: &Path, items: Vec<T>) {
        self.data.insert(book_key(book), items);
    }

    pub fn save(&self) {
        if self.path.as_os_str().is_empty() {
            return;
        }
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.data) {
            let _ = std::fs::write(&self.path, json);
        }
    }
}

pub type PostitStore = BookStore<Postit>;
pub type BookmarkStore = BookStore<Bookmark>;
pub type HighlightStore = BookStore<Highlight>;
pub type SavedPointStore = BookStore<SavedPoint>;

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
    fn saved_points_roundtrip_by_book() {
        let dir = std::env::temp_dir().join(format!("pagething-points-{}", std::process::id()));
        let path = dir.join("saved_points.json");
        let book = PathBuf::from("/tmp/points-book.epub");
        let point = SavedPoint {
            page: 12,
            view: ViewState {
                pan_x: 135.0,
                pan_y: -24.0,
                zoom: 1.75,
            },
            saved_at: 1_700_000_000,
        };
        let mut store = SavedPointStore::load_from(path.clone());
        store.set(&book, vec![point]);
        store.save();

        let reloaded = SavedPointStore::load_from(path);
        assert_eq!(reloaded.get(&book), &[point]);
        assert!(reloaded.get(Path::new("/tmp/another-book.epub")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_file_yields_empty_store() {
        let store = ViewStore::load_from(PathBuf::from("/nonexistent-xyz/views.json"));
        assert_eq!(store.get(Path::new("/book.epub")), None);
    }

    #[test]
    fn title_store_roundtrips_and_removes() {
        let dir = std::env::temp_dir().join(format!("pagething-titles-{}", std::process::id()));
        let path = dir.join("titles.json");
        let book = PathBuf::from("/tmp/renamed-book.epub");

        let mut store = TitleStore::load_from(path.clone());
        store.set(&book, "My Custom Title".to_owned());
        store.save();

        let mut reloaded = TitleStore::load_from(path.clone());
        assert_eq!(reloaded.get(&book), Some("My Custom Title"));
        assert_eq!(reloaded.get(Path::new("/tmp/other.epub")), None);

        reloaded.remove(&book);
        reloaded.save();
        let cleared = TitleStore::load_from(path);
        assert_eq!(cleared.get(&book), None);

        let _ = std::fs::remove_dir_all(&dir);
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
            s.page_bg = [12, 34, 56];
            s.page_font = [78, 90, 123];
            s.page_texture = PageTexture::LinenWeave;
            s.app_bg_mode = AppBgMode::Custom;
            s.app_bg = [150, 160, 170];
            s.justify = Justify::Center;
            s.ui_scale = 1.5;
            s.font_size = 20.0;
            s.page_gap = 80.0;
            s.page_ratio = PageRatio::FiveEight;
            s.book_font = BookFont::Sans;
            s.line_spacing = LineSpacing::Spacious;
            s.toolbar = false;
        });

        let reloaded = SettingsStore::load_from(path.clone());
        let s = reloaded.settings();
        assert_eq!(s.columns, 6);
        assert_eq!(s.margin, 72.0);
        assert_eq!(s.theme, ThemePref::Dark);
        assert_eq!(s.page_mode, PageMode::DarkPages);
        assert_eq!(s.page_bg, [12, 34, 56]);
        assert_eq!(s.page_font, [78, 90, 123]);
        assert_eq!(s.page_texture, PageTexture::LinenWeave);
        assert_eq!(s.app_bg_mode, AppBgMode::Custom);
        assert_eq!(s.app_bg, [150, 160, 170]);
        assert_eq!(s.justify, Justify::Center);
        assert_eq!(s.ui_scale, 1.5);
        assert_eq!(s.font_size, 20.0);
        assert_eq!(s.page_gap, 80.0);
        assert_eq!(s.page_ratio, PageRatio::FiveEight);
        assert_eq!(s.book_font, BookFont::Sans);
        assert_eq!(s.line_spacing, LineSpacing::Spacious);
        assert!(!s.toolbar);

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
            r#"{"columns":0,"margin":999.0,"ui_scale":99.0,"font_size":99.0,"page_gap":999.0}"#,
        )
        .unwrap();

        let store = SettingsStore::load_from(path);
        let s = store.settings();
        assert_eq!(s.columns, 1);
        assert_eq!(s.margin, MAX_MARGIN);
        assert_eq!(s.ui_scale, MAX_UI_SCALE);
        assert_eq!(s.font_size, MAX_FONT_SIZE);
        assert_eq!(s.page_gap, MAX_PAGE_GAP);
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

        let json = serde_json::to_string(&Settings {
            page_mode: PageMode::Sepia,
            ..Settings::default()
        })
        .unwrap();
        assert!(json.contains("\"page_mode\":\"sepia\""));
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.page_mode, PageMode::Sepia);
    }

    #[test]
    fn new_settings_default_for_legacy_files() {
        let settings: Settings = serde_json::from_str(r#"{"columns":6}"#).unwrap();
        assert_eq!(settings.page_ratio, PageRatio::TwoThree);
        assert_eq!(settings.page_texture, PageTexture::FineGrain);
        assert_eq!(settings.justify, Justify::Justified);
        assert!(settings.toolbar);
    }

    #[test]
    fn page_texture_modes_serialize_snake_case() {
        for (texture, serialized) in [
            (PageTexture::Plain, "plain"),
            (PageTexture::FineGrain, "fine_grain"),
            (PageTexture::PaperFibers, "paper_fibers"),
            (PageTexture::LinenWeave, "linen_weave"),
        ] {
            let json = serde_json::to_string(&texture).unwrap();
            assert_eq!(json, format!("\"{serialized}\""));
            assert_eq!(serde_json::from_str::<PageTexture>(&json).unwrap(), texture);
        }
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

    #[test]
    fn book_store_roundtrips_notes_by_book() {
        let dir = std::env::temp_dir().join(format!("pagething-notes-{}", std::process::id()));
        let path = dir.join("postits.json");
        let book = PathBuf::from("/tmp/notes-book.epub");
        let note = Postit::new(3, "remember this".to_owned(), None);

        let mut store = PostitStore::load_from(path.clone());
        store.set(&book, vec![note.clone()]);
        store.save();

        let reloaded = PostitStore::load_from(path);
        assert_eq!(reloaded.get(&book), &[note]);
        assert!(reloaded.get(Path::new("/tmp/another-book.epub")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn highlight_store_roundtrips_and_defaults_created_at() {
        let dir = std::env::temp_dir().join(format!("pagething-hl-{}", std::process::id()));
        let path = dir.join("highlights.json");
        let book = PathBuf::from("/tmp/hl-book.epub");
        let hl = Highlight {
            page: 2,
            start: 10,
            end: 25,
            anchor: None,
            created_at: 1_700_000_000,
        };

        let mut store = HighlightStore::load_from(path.clone());
        store.set(&book, vec![hl.clone()]);
        store.save();

        let reloaded = HighlightStore::load_from(path.clone());
        assert_eq!(reloaded.get(&book), &[hl]);

        // legacy JSON without created_at
        std::fs::write(
            &path,
            r#"{"/tmp/hl-book.epub":[{"page":1,"start":0,"end":5}]}"#,
        )
        .unwrap();
        let legacy = HighlightStore::load_from(path);
        assert_eq!(
            legacy.get(&book),
            &[Highlight {
                page: 1,
                start: 0,
                end: 5,
                anchor: None,
                created_at: 0
            }]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn postit_legacy_json_loads_with_default_meta() {
        let json = r#"{"page":3,"text":"old note","highlight":null}"#;
        let note: Postit = serde_json::from_str(json).unwrap();
        assert_eq!(note.page, 3);
        assert_eq!(note.text, "old note");
        assert_eq!(note.created_at, 0);
        assert_eq!(note.updated_at, 0);
        assert!(note.replies.is_empty());
        assert!(!note.collapsed, "legacy notes default to expanded");
    }

    #[test]
    fn bookmark_store_reads_legacy_page_numbers_and_anchored_bookmarks() {
        let stored: Vec<Bookmark> = serde_json::from_str(
            r#"[3,{"page":7,"anchor":{"quote":"chapter","before":"","after":" one"}}]"#,
        )
        .unwrap();
        assert_eq!(
            stored[0],
            Bookmark {
                page: 3,
                anchor: None
            }
        );
        assert_eq!(stored[1].page, 7);
        assert_eq!(stored[1].anchor.as_ref().unwrap().quote, "chapter");
    }

    #[test]
    fn fmt_ts_formats_known_timestamp() {
        // 2023-11-14 22:13:20 UTC
        assert_eq!(fmt_ts(1_700_000_000), "14 Nov 22:13");
        assert_eq!(fmt_ts(0), "—");
    }
}
