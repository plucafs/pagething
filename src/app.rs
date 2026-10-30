use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use eframe::egui::{self, Color32, Key, Modifiers, PointerButton, Response, ViewportCommand};

use crate::book::blocks::Block;
use crate::book::paginate;
use crate::book::prepare::{self, PreparedBook};
use crate::canvas::{View, grid};
use crate::features::dialogs::{self, Dialog, Outcome};
use crate::features::search::{self, Hit};
use crate::features::selection::{self, Selection};
use crate::persist::{Settings, SettingsStore, ThemePref, ViewState, ViewStore};
use crate::render::layout::{self, DisplayCache, LaidBlock, Margins};
use crate::render::page_view::{self, PageView};
use crate::render::theme::{self, PageColors};

const MAX_RECENTS: usize = 10;

struct LoadedBook {
    title: Option<String>,
    cover: Option<Arc<egui::TextureHandle>>,
    laid: Vec<LaidBlock>,
    pages: Vec<paginate::Page>,
    blocks: Vec<Block>,
    textures: HashMap<String, Arc<egui::TextureHandle>>,
}

enum Phase {
    Empty {
        error: Option<String>,
    },
    Loading {
        rx: Receiver<Result<PreparedBook, String>>,
    },
    Ready(LoadedBook),
}

pub struct PagethingApp {
    view: View,
    columns: usize,
    phase: Phase,
    recents: Vec<PathBuf>,
    want_open: bool,
    display_cache: DisplayCache,
    view_store: ViewStore,
    settings: SettingsStore,
    current_book: Option<PathBuf>,
    dialog: Option<Dialog>,
    pending_jump: Option<usize>,
    pending_navigation: Option<bool>,
    pending_selection_search: Option<String>,
    text_rendering: fastframe_text::TextRendering,
    selection: Option<Selection>,
    selection_drag: bool,
    search_hits: Vec<Hit>,
    search_query: String,
    search_index: usize,
}

impl PagethingApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let text_rendering = fastframe_text::detect();
        crate::render::fonts::install(&cc.egui_ctx, &text_rendering);
        let settings = SettingsStore::load();
        apply_theme_pref(&cc.egui_ctx, settings.settings().theme);
        cc.egui_ctx.set_zoom_factor(settings.settings().ui_scale);
        let mut app = Self {
            view: View::default(),
            columns: settings.settings().columns,
            phase: Phase::Empty { error: None },
            recents: load_recents(),
            want_open: false,
            display_cache: DisplayCache::default(),
            view_store: ViewStore::load(),
            settings,
            current_book: None,
            dialog: None,
            pending_jump: None,
            pending_navigation: None,
            pending_selection_search: None,
            text_rendering,
            selection: None,
            selection_drag: false,
            search_hits: Vec::new(),
            search_query: String::new(),
            search_index: 0,
        };
        if let Some(latest) = app.recents.first().cloned()
            && latest.exists()
        {
            app.open_path(latest);
        }
        app
    }

    fn open_path(&mut self, path: PathBuf) {
        self.stash_view();
        self.display_cache.clear();
        self.selection = None;
        self.selection_drag = false;
        self.search_hits.clear();
        self.search_query.clear();
        self.search_index = 0;
        self.view = self
            .view_store
            .get(&path)
            .map(|s| View {
                pan: egui::vec2(s.pan_x, s.pan_y),
                zoom: s.zoom,
            })
            .unwrap_or_default();

        let (tx, rx) = mpsc::channel();
        let load_path = path.clone();
        std::thread::spawn(move || {
            let result = prepare::prepare(&load_path).map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        self.phase = Phase::Loading { rx };
        self.current_book = Some(path);
    }

    fn stash_view(&mut self) {
        let Some(book) = self.current_book.clone() else {
            return;
        };
        self.view_store.set(
            &book,
            ViewState {
                pan_x: self.view.pan.x,
                pan_y: self.view.pan.y,
                zoom: self.view.zoom,
            },
        );
        self.view_store.save();
    }

    fn remember(&mut self, path: PathBuf) {
        self.recents.retain(|p| p != &path);
        self.recents.insert(0, path);
        self.recents.truncate(MAX_RECENTS);
        save_recents(&self.recents);
    }

    fn build_book(ctx: &egui::Context, raw: PreparedBook, settings: Settings) -> LoadedBook {
        let mut textures = HashMap::new();
        for (name, image) in raw.images {
            let handle = layout::load_texture(ctx, &name, &image);
            textures.insert(name, Arc::new(handle));
        }
        let cover = raw
            .cover
            .map(|image| Arc::new(layout::load_texture(ctx, "cover", &image)));

        let margins = Margins::uniform(settings.margin);
        let width = layout::content_width(&margins);
        let height = layout::content_height(&margins);
        let laid = layout::layout_blocks(
            ctx,
            &raw.blocks,
            &textures,
            width,
            height,
            &PageColors::for_mode(settings.page_mode),
        );
        let metrics: Vec<_> = laid.iter().map(|b| b.metrics()).collect();
        let mut pages = paginate::paginate(&metrics, height, cover.is_some());
        layout::attach_page_texts(&mut pages, &laid);

        LoadedBook {
            title: raw.title,
            cover,
            laid,
            pages,
            blocks: raw.blocks,
            textures,
        }
    }

    fn poll_loading(&mut self, ctx: &egui::Context) {
        let mut outcome: Option<Result<PreparedBook, String>> = None;
        if let Phase::Loading { rx } = &self.phase {
            match rx.try_recv() {
                Ok(result) => outcome = Some(result),
                Err(TryRecvError::Disconnected) => {
                    outcome = Some(Err("book loading failed".to_owned()))
                }
                Err(TryRecvError::Empty) => {}
            }
        }

        if let Some(result) = outcome {
            match result {
                Ok(raw) => {
                    let book = Self::build_book(ctx, raw, self.settings.settings());
                    self.phase = Phase::Ready(book);
                }
                Err(error) => self.phase = Phase::Empty { error: Some(error) },
            }
        }
    }

    fn rebuild_ready_book(&mut self, ctx: &egui::Context) {
        let Phase::Ready(book) = &mut self.phase else {
            return;
        };
        let settings = self.settings.settings();
        let margins = Margins::uniform(settings.margin);
        let width = layout::content_width(&margins);
        let height = layout::content_height(&margins);
        book.laid = layout::layout_blocks(
            ctx,
            &book.blocks,
            &book.textures,
            width,
            height,
            &PageColors::for_mode(settings.page_mode),
        );
        let metrics: Vec<_> = book.laid.iter().map(|b| b.metrics()).collect();
        book.pages = paginate::paginate(&metrics, height, book.cover.is_some());
        layout::attach_page_texts(&mut book.pages, &book.laid);
        self.display_cache.clear();
        self.selection = None;
        self.selection_drag = false;
        self.search_hits.clear();
        self.search_query.clear();
        self.search_index = 0;
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::O)) {
            self.want_open = true;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::N)) {
            spawn_new_instance();
        }
        if dialogs::consume_shortcut(ctx, Key::G) && matches!(self.phase, Phase::Ready(_)) {
            self.dialog = Some(Dialog::Jump {
                input: String::new(),
            });
        }
        if dialogs::consume_shortcut(ctx, Key::L) {
            self.dialog = Some(Dialog::Columns {
                input: self.columns.to_string(),
            });
        }
        if dialogs::consume_shortcut(ctx, Key::Comma) {
            self.dialog = Some(Dialog::Settings {
                draft: self.settings.settings(),
            });
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::Q)) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        if self.dialog.is_none()
            && !ctx.any_popup_open()
            && let Phase::Ready(book) = &self.phase
        {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Home)) {
                self.pending_jump = Some(1);
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::End)) {
                self.pending_jump = Some(book.pages.len());
            }
        }
        if dialogs::consume_shortcut(ctx, Key::F) {
            self.dialog = Some(Dialog::Search {
                input: String::new(),
            });
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::C))
            && let Some(sel) = &self.selection
            && let Phase::Ready(book) = &self.phase
            && let Some(page) = book.pages.get(sel.page)
        {
            let text = selection::copy_text(page, &book.laid, sel);
            if !text.is_empty() {
                ctx.copy_text(text);
            }
        }

        if self.dialog.is_none() && !ctx.any_popup_open() {
            let has_selection = self.selection.is_some_and(|sel| !sel.is_empty());
            if has_selection
                && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::C))
                && let (Some(sel), Phase::Ready(book)) = (&self.selection, &self.phase)
                && let Some(page) = book.pages.get(sel.page)
            {
                let text = selection::copy_text(page, &book.laid, sel);
                if !text.is_empty() {
                    ctx.copy_text(text);
                }
            } else if has_selection
                && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F))
                && let (Some(sel), Phase::Ready(book)) = (&self.selection, &self.phase)
                && let Some(page) = book.pages.get(sel.page)
            {
                let query = selection::copy_text(page, &book.laid, sel);
                self.dialog = Some(Dialog::Search {
                    input: query.clone(),
                });
                self.pending_selection_search = Some(query);
            } else if has_selection && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Q)) {
                self.selection = None;
                self.selection_drag = false;
            }

            if self.dialog.is_none() && matches!(self.phase, Phase::Ready(_)) {
                let backward = ctx.input_mut(|i| {
                    i.consume_key(Modifiers::NONE, Key::Z)
                        || i.consume_key(Modifiers::NONE, Key::ArrowLeft)
                        || i.consume_key(Modifiers::NONE, Key::PageUp)
                });
                let forward = ctx.input_mut(|i| {
                    i.consume_key(Modifiers::NONE, Key::X)
                        || i.consume_key(Modifiers::NONE, Key::ArrowRight)
                        || i.consume_key(Modifiers::NONE, Key::PageDown)
                });
                if backward {
                    self.pending_navigation = Some(false);
                } else if forward {
                    self.pending_navigation = Some(true);
                }
            }
        }
        self.view.handle_shortcuts(ctx);
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("menu_bar").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open…").clicked() {
                        ui.close();
                        self.want_open = true;
                    }
                    if ui
                        .add(egui::Button::new("New Window").shortcut_text("Ctrl+N"))
                        .clicked()
                    {
                        ui.close();
                        spawn_new_instance();
                    }
                    ui.add_enabled_ui(!self.recents.is_empty(), |ui| {
                        ui.menu_button("Recent", |ui| {
                            let mut picked: Option<PathBuf> = None;
                            for path in &self.recents {
                                let label = path
                                    .file_name()
                                    .map(|n| n.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| path.display().to_string());
                                if ui.button(label).clicked() {
                                    picked = Some(path.clone());
                                    ui.close();
                                }
                            }
                            if let Some(path) = picked {
                                self.open_path(path);
                            }
                        });
                    });
                    ui.separator();
                    if ui
                        .add(egui::Button::new("Settings…").shortcut_text("Ctrl+,"))
                        .clicked()
                    {
                        ui.close();
                        self.dialog = Some(Dialog::Settings {
                            draft: self.settings.settings(),
                        });
                    }
                    ui.separator();
                    if ui
                        .add(egui::Button::new("Quit").shortcut_text("Ctrl+Q"))
                        .clicked()
                    {
                        ui.send_viewport_cmd(ViewportCommand::Close);
                    }
                });
                ui.menu_button("View", |ui| {
                    ui.add_enabled_ui(matches!(self.phase, Phase::Ready(_)), |ui| {
                        if ui
                            .add(egui::Button::new("Go to page…").shortcut_text("Ctrl+G"))
                            .clicked()
                        {
                            ui.close();
                            self.dialog = Some(Dialog::Jump {
                                input: String::new(),
                            });
                        }
                    });
                    if ui
                        .add(egui::Button::new("Columns…").shortcut_text("Ctrl+L"))
                        .clicked()
                    {
                        ui.close();
                        self.dialog = Some(Dialog::Columns {
                            input: self.columns.to_string(),
                        });
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Shortcuts").clicked() {
                        ui.close();
                        self.dialog = Some(Dialog::Shortcuts);
                    }
                    if ui.button("About").clicked() {
                        ui.close();
                        self.dialog = Some(Dialog::Help);
                    }
                });
            });
        });
    }

    fn title_bar(&mut self, ui: &mut egui::Ui) {
        let Phase::Ready(book) = &self.phase else {
            return;
        };
        let Some(title) = book.title.clone() else {
            return;
        };
        egui::Panel::top("book_title").show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(2.0);
                ui.label(egui::RichText::new(title).size(13.0).strong());
                ui.add_space(2.0);
            });
        });
    }

    fn run_file_dialog(&mut self) {
        if !self.want_open {
            return;
        }
        self.want_open = false;
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("EPUB", &["epub"])
            .pick_file()
        {
            self.remember(path.clone());
            self.open_path(path);
        }
    }

    fn jump_to_page(&mut self, page: usize, canvas_rect: egui::Rect) {
        if page == 0 {
            return;
        }
        self.selection = None;
        self.selection_drag = false;
        self.center_page(page, canvas_rect);
    }

    fn center_page(&mut self, page: usize, canvas_rect: egui::Rect) {
        if page == 0 {
            return;
        }
        let world_rect = grid::page_rect(page - 1, self.columns, grid::GAP);
        let world_center = world_rect.center();
        self.view.pan = canvas_rect.center() - world_center * self.view.zoom;
    }

    fn navigate_page(&mut self, forward: bool, canvas_rect: egui::Rect) {
        let Phase::Ready(book) = &self.phase else {
            return;
        };
        let page_count = book.pages.len();
        let center = self.view.to_world(canvas_rect.center()).to_pos2();
        let Some(current) =
            grid::page_index_at_or_near(center, self.columns, grid::GAP, page_count)
        else {
            return;
        };
        let Some(target) = grid::adjacent_page_index(current, page_count, forward) else {
            return;
        };
        self.jump_to_page(target + 1, canvas_rect);
    }

    /// Enter (forward) / Shift+Enter (backward): re-search when the query
    /// changed, otherwise advance the index with wrap-around, then jump.
    fn goto_hit(
        &mut self,
        forward: bool,
        query: &str,
        canvas_rect: egui::Rect,
        preserve_selection: bool,
    ) {
        if query != self.search_query {
            if let Phase::Ready(book) = &self.phase {
                self.search_hits = search::search(&book.pages, query);
                self.search_query = query.to_owned();
                self.search_index = 0;
            }
        } else if !self.search_hits.is_empty() {
            let len = self.search_hits.len();
            self.search_index = if forward {
                (self.search_index + 1) % len
            } else {
                (self.search_index + len - 1) % len
            };
        }
        if let Some(hit) = self.search_hits.get(self.search_index) {
            let page = hit.page + 1;
            if preserve_selection {
                self.center_page(page, canvas_rect);
            } else {
                self.jump_to_page(page, canvas_rect);
            }
        }
    }

    /// Screen position → `(page index, piece char loc)` over visible text.
    fn loc_at(&self, canvas_rect: egui::Rect, pos: egui::Pos2) -> Option<(usize, selection::Loc)> {
        let Phase::Ready(book) = &self.phase else {
            return None;
        };
        if !canvas_rect.contains(pos) {
            return None;
        }
        let margins = Margins::uniform(self.settings.settings().margin);
        let world = self.view.to_world(pos);
        for (i, page) in book.pages.iter().enumerate() {
            let world_rect = grid::page_rect(i, self.columns, grid::GAP);
            if !world_rect.contains(world.to_pos2()) {
                continue;
            }
            let page_pos = world - world_rect.min.to_vec2();
            if let Some(loc) = selection::hit_test(page, &book.laid, margins, page_pos) {
                return Some((i, loc));
            }
        }
        None
    }

    fn handle_selection_input(&mut self, canvas_rect: egui::Rect, response: &Response) {
        if response.drag_started_by(PointerButton::Primary) {
            let hit = response
                .interact_pointer_pos()
                .and_then(|pos| self.loc_at(canvas_rect, pos));
            match hit {
                Some((page, loc)) => {
                    self.selection = Some(Selection::single(page, loc));
                    self.selection_drag = true;
                }
                None => {
                    self.selection = None;
                    self.selection_drag = false;
                }
            }
        } else if response.dragged_by(PointerButton::Primary) && self.selection_drag {
            if let Some(pos) = response.interact_pointer_pos()
                && let Some((page, loc)) = self.loc_at(canvas_rect, pos)
                && let Some(sel) = &mut self.selection
                && sel.page == page
            {
                sel.cursor = loc;
            }
        } else if response.drag_stopped_by(PointerButton::Primary) {
            self.selection_drag = false;
        }
    }

    fn apply_dialog(&mut self, canvas_rect: egui::Rect, ctx: &egui::Context) {
        let Some(dialog) = &mut self.dialog else {
            return;
        };
        let page_count = match &self.phase {
            Phase::Ready(book) => book.pages.len(),
            _ => 0,
        };
        let search_input = match dialog {
            Dialog::Search { input } => input.clone(),
            _ => String::new(),
        };
        let outcome = dialogs::dialog_ui(ctx, dialog, page_count, &self.search_hits);
        match outcome {
            Outcome::Keep => {}
            Outcome::Close => self.dialog = None,
            Outcome::Jumped(page) => {
                self.dialog = None;
                self.jump_to_page(page, canvas_rect);
            }
            Outcome::Columns(columns) => {
                self.dialog = None;
                self.columns = columns;
                self.settings.update(|s| s.columns = columns);
            }
            Outcome::Search(query) => self.goto_hit(true, &query, canvas_rect, false),
            Outcome::SearchPrev => self.goto_hit(false, &search_input, canvas_rect, false),
            Outcome::SearchJump(index) => {
                if let Some(hit) = self.search_hits.get(index) {
                    self.search_index = index;
                    let page = hit.page + 1;
                    self.jump_to_page(page, canvas_rect);
                }
            }
            Outcome::Settings(changed) => {
                let prev = self.settings.settings();
                self.settings.update(|s| *s = changed);
                if changed.theme != prev.theme {
                    apply_theme_pref(ctx, changed.theme);
                }
                if changed.ui_scale != prev.ui_scale {
                    ctx.set_zoom_factor(changed.ui_scale);
                }
                if changed.margin != prev.margin || changed.page_mode != prev.page_mode {
                    self.rebuild_ready_book(ctx);
                }
            }
        }
    }

    fn paint_canvas(&mut self, ui: &mut egui::Ui) {
        let canvas_rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(canvas_rect, egui::Sense::click_and_drag());
        self.view.handle_input(ui.ctx(), canvas_rect, &response);

        if let Some(page) = self.pending_jump.take() {
            self.jump_to_page(page, canvas_rect);
        }
        if let Some(forward) = self.pending_navigation.take() {
            self.navigate_page(forward, canvas_rect);
        }
        if let Some(query) = self.pending_selection_search.take() {
            self.goto_hit(true, &query, canvas_rect, true);
        }
        self.handle_selection_input(canvas_rect, &response);

        let painter = ui.painter().with_clip_rect(canvas_rect);
        painter.rect_filled(canvas_rect, 0.0, theme::canvas_bg(ui.ctx().theme()));

        match &self.phase {
            Phase::Empty { error } => {
                let (message, color) = match error {
                    Some(e) => (
                        format!("Could not open book: {e}"),
                        Color32::from_rgb(220, 100, 100),
                    ),
                    None => (
                        "Ctrl+O to open an EPUB".to_owned(),
                        theme::hint_color(ui.ctx().theme()),
                    ),
                };
                painter.text(
                    canvas_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    message,
                    egui::FontId::proportional(18.0),
                    color,
                );
            }
            Phase::Loading { .. } => {
                painter.text(
                    canvas_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Loading…",
                    egui::FontId::proportional(18.0),
                    theme::loading_color(ui.ctx().theme()),
                );
            }
            Phase::Ready(book) => {
                let ctx = ui.ctx().clone();
                let font_size = ctx.fonts(|f| f.font_image_size());
                let font_uv = egui::Vec2::new(
                    1.0 / font_size[0].max(1) as f32,
                    1.0 / font_size[1].max(1) as f32,
                );
                let settings = self.settings.settings();
                let margins = Margins::uniform(settings.margin);
                let colors = PageColors::for_mode(settings.page_mode);
                for (i, page) in book.pages.iter().enumerate() {
                    let world_rect = grid::page_rect(i, self.columns, grid::GAP);
                    let screen_rect = self.view.to_screen_rect(world_rect);
                    if !canvas_rect.intersects(screen_rect) {
                        continue;
                    }
                    let mut highlights: Vec<(usize, usize, usize, Color32)> = Vec::new();
                    for (hi, hit) in self.search_hits.iter().enumerate() {
                        if hit.page == i {
                            let color = if hi == self.search_index {
                                page_view::SEARCH_CURRENT_HL
                            } else {
                                page_view::SEARCH_HL
                            };
                            highlights.extend(
                                selection::piece_ranges(page, hit.start, hit.end)
                                    .into_iter()
                                    .map(|(ii, a, b)| (ii, a, b, color)),
                            );
                        }
                    }
                    if let Some(sel) = &self.selection
                        && sel.page == i
                    {
                        highlights.extend(
                            selection::selection_spans(page, sel)
                                .into_iter()
                                .map(|(ii, a, b)| (ii, a, b, page_view::SELECTION_HL)),
                        );
                    }
                    let pv = PageView {
                        page,
                        laid: &book.laid,
                        cover: book.cover.as_ref(),
                        number: i + 1,
                        font_uv,
                        margins,
                        colors,
                        highlights: &highlights,
                    };
                    page_view::paint_page(
                        &painter,
                        &ctx,
                        &mut self.display_cache,
                        &self.view,
                        screen_rect,
                        &pv,
                    );
                }
            }
        }

        let ctx = ui.ctx().clone();
        self.apply_dialog(canvas_rect, &ctx);
        if matches!(self.phase, Phase::Loading { .. }) {
            ctx.request_repaint();
        }
    }
}

impl eframe::App for PagethingApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let rendering = self.text_rendering;
        ctx.all_styles_mut(|style| rendering.apply_to_visuals(&mut style.visuals));
        self.handle_shortcuts(&ctx);
        self.menu_bar(ui);
        self.title_bar(ui);
        self.run_file_dialog();
        self.poll_loading(&ctx);
        self.paint_canvas(ui);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.stash_view();
    }
}

fn spawn_new_instance() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).spawn();
    }
}

fn apply_theme_pref(ctx: &egui::Context, pref: ThemePref) {
    ctx.set_theme(match pref {
        ThemePref::Light => egui::ThemePreference::Light,
        ThemePref::Dark => egui::ThemePreference::Dark,
        ThemePref::System => egui::ThemePreference::System,
    });
}

fn recents_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("pagething").join("recents.json"))
}

fn load_recents() -> Vec<PathBuf> {
    let Some(path) = recents_path() else {
        return Vec::new();
    };
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<PathBuf>>(&s).ok())
        .unwrap_or_default()
}

fn save_recents(recents: &[PathBuf]) {
    let Some(path) = recents_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(recents) {
        let _ = std::fs::write(path, json);
    }
}

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 860.0])
            .with_title("pagething"),
        ..Default::default()
    };
    eframe::run_native(
        "pagething",
        options,
        Box::new(|cc| Ok(Box::new(PagethingApp::new(cc)))),
    )
}
