use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};

use eframe::egui::{self, Color32, Key, Modifiers, PointerButton, Response, ViewportCommand};

use crate::book::blocks::Block;
use crate::book::paginate;
use crate::book::prepare::{self, PreparedBook};
use crate::book::progress::{LoadProgress, LoadStage};
use crate::canvas::{View, grid};
use crate::features::dialogs::{self, Dialog, Outcome};
use crate::features::search::{self, Hit};
use crate::features::selection::{self, Selection};
use crate::persist::{
    BookmarkStore, Postit, PostitStore, Settings, SettingsStore, ThemePref, ViewState, ViewStore,
};
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
    chapter_starts: Vec<usize>,
    image_sizes: HashMap<String, egui::Vec2>,
    textures: HashMap<String, Arc<egui::TextureHandle>>,
    image_rx: Option<Receiver<ImageDecodeMessage>>,
    image_cancel: Arc<AtomicBool>,
}

impl Drop for LoadedBook {
    fn drop(&mut self) {
        self.image_cancel.store(true, Ordering::Relaxed);
    }
}

enum Phase {
    Empty {
        error: Option<String>,
    },
    Loading {
        rx: Receiver<LoadMessage>,
        progress: LoadProgress,
        prepared: Option<PreparedBook>,
    },
    Ready(LoadedBook),
}

enum LoadMessage {
    Progress(LoadProgress),
    Done(Result<PreparedBook, String>),
}

enum ImageDecodeMessage {
    Decoded(String, egui::ColorImage),
    Finished,
}

pub struct PagethingApp {
    view: View,
    columns: usize,
    phase: Phase,
    recents: Vec<PathBuf>,
    want_open: bool,
    display_cache: DisplayCache,
    view_store: ViewStore,
    postit_store: PostitStore,
    bookmark_store: BookmarkStore,
    postits: Vec<Postit>,
    bookmarks: Vec<usize>,
    settings: SettingsStore,
    current_book: Option<PathBuf>,
    dialog: Option<Dialog>,
    pending_jump: Option<usize>,
    pending_navigation: Option<bool>,
    pending_selection_search: Option<String>,
    pending_postit: bool,
    pending_bookmark: bool,
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
            postit_store: PostitStore::load("postits.json"),
            bookmark_store: BookmarkStore::load("bookmarks.json"),
            postits: Vec::new(),
            bookmarks: Vec::new(),
            settings,
            current_book: None,
            dialog: None,
            pending_jump: None,
            pending_navigation: None,
            pending_selection_search: None,
            pending_postit: false,
            pending_bookmark: false,
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
        self.postits = self.postit_store.get(&path).to_vec();
        self.bookmarks = self.bookmark_store.get(&path).to_vec();
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
            let result = prepare::prepare_with_progress(&load_path, &mut |progress| {
                let _ = tx.send(LoadMessage::Progress(progress));
            });
            let _ = tx.send(LoadMessage::Done(result));
        });
        self.phase = Phase::Loading {
            rx,
            progress: LoadProgress {
                stage: LoadStage::Opening,
                current: 0,
                total: 1,
            },
            prepared: None,
        };
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
        let image_sizes: HashMap<_, _> = raw
            .images
            .iter()
            .map(|(name, image)| (name.clone(), image.size))
            .collect();
        let image_bytes: Vec<_> = raw
            .images
            .into_iter()
            .map(|(name, image)| (name, image.bytes))
            .collect();
        let cover = raw
            .cover
            .map(|image| Arc::new(layout::load_texture(ctx, "cover", &image)));
        let has_cover = cover.is_some();

        let margins = Margins::uniform(settings.margin);
        let width = layout::content_width(&margins);
        let height = layout::content_height(&margins);
        let laid = layout::layout_blocks(
            ctx,
            &raw.blocks,
            &raw.chapter_starts,
            &image_sizes,
            settings.font_size,
            width,
            height,
            &PageColors::for_mode(settings.page_mode),
        );
        let metrics: Vec<_> = laid.iter().map(|b| b.metrics()).collect();
        let mut pages = paginate::paginate(&metrics, height, has_cover);
        layout::attach_page_texts(&mut pages, &laid);

        let image_cancel = Arc::new(AtomicBool::new(false));
        let image_rx = if image_bytes.is_empty() {
            None
        } else {
            let (tx, rx) = mpsc::channel();
            let cancel = Arc::clone(&image_cancel);
            std::thread::spawn(move || {
                for (name, bytes) in image_bytes {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    if let Some(image) = layout::decode_image(&bytes)
                        && tx.send(ImageDecodeMessage::Decoded(name, image)).is_err()
                    {
                        break;
                    }
                }
                let _ = tx.send(ImageDecodeMessage::Finished);
            });
            Some(rx)
        };

        LoadedBook {
            title: raw.title,
            cover,
            laid,
            pages,
            blocks: raw.blocks,
            chapter_starts: raw.chapter_starts,
            image_sizes,
            textures: HashMap::new(),
            image_rx,
            image_cancel,
        }
    }

    fn poll_image_decode(&mut self, ctx: &egui::Context) {
        let Phase::Ready(book) = &mut self.phase else {
            return;
        };
        let Some(rx) = book.image_rx.as_ref() else {
            return;
        };

        let mut decoded = 0;
        let mut finished = false;
        while decoded < 2 {
            match rx.try_recv() {
                Ok(ImageDecodeMessage::Decoded(name, image)) => {
                    let texture = layout::load_texture(ctx, &name, &image);
                    book.textures.insert(name, Arc::new(texture));
                    decoded += 1;
                }
                Ok(ImageDecodeMessage::Finished) => {
                    finished = true;
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    finished = true;
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }
        if finished {
            book.image_rx = None;
        } else {
            ctx.request_repaint();
        }
    }

    fn poll_loading(&mut self, ctx: &egui::Context) {
        let prepared = match &mut self.phase {
            Phase::Loading { prepared, .. } => prepared.take(),
            _ => None,
        };
        if let Some(raw) = prepared {
            let book = Self::build_book(ctx, raw, self.settings.settings());
            self.phase = Phase::Ready(book);
            return;
        }

        let mut outcome = None;
        let mut latest_progress = None;
        let mut disconnected = false;
        if let Phase::Loading { rx, .. } = &self.phase {
            loop {
                match rx.try_recv() {
                    Ok(LoadMessage::Progress(progress)) => latest_progress = Some(progress),
                    Ok(LoadMessage::Done(result)) => outcome = Some(result),
                    Err(TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
        }

        if let Phase::Loading { progress, .. } = &mut self.phase
            && let Some(latest) = latest_progress
        {
            *progress = latest;
        }

        if let Some(result) = outcome {
            match result {
                Ok(prepared) => {
                    if let Phase::Loading {
                        progress,
                        prepared: pending,
                        ..
                    } = &mut self.phase
                    {
                        *progress = LoadProgress {
                            stage: LoadStage::Typesetting,
                            current: 0,
                            total: 1,
                        };
                        *pending = Some(prepared);
                    }
                }
                Err(error) => self.phase = Phase::Empty { error: Some(error) },
            }
        } else if disconnected {
            self.phase = Phase::Empty {
                error: Some("book loading failed".to_owned()),
            };
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
            &book.chapter_starts,
            &book.image_sizes,
            settings.font_size,
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
        if self.dialog.is_none()
            && matches!(self.phase, Phase::Ready(_))
            && dialogs::consume_shortcut(ctx, Key::V)
        {
            self.dialog = Some(Dialog::Bookmarks);
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
            if matches!(self.phase, Phase::Ready(_)) {
                if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::A)) {
                    self.pending_postit = true;
                }
                if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::V)) {
                    self.pending_bookmark = true;
                }
            }
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
        let world_rect = grid::page_rect(page - 1, self.columns, self.settings.settings().page_gap);
        let world_center = world_rect.center();
        self.view.pan = canvas_rect.center() - world_center * self.view.zoom;
    }

    fn current_page(&self, canvas_rect: egui::Rect) -> Option<usize> {
        let Phase::Ready(book) = &self.phase else {
            return None;
        };
        let center = self.view.to_world(canvas_rect.center()).to_pos2();
        grid::page_index_at_or_near(
            center,
            self.columns,
            self.settings.settings().page_gap,
            book.pages.len(),
        )
        .map(|index| index + 1)
    }

    fn navigate_page(&mut self, forward: bool, canvas_rect: egui::Rect) {
        let Phase::Ready(book) = &self.phase else {
            return;
        };
        let page_count = book.pages.len();
        let center = self.view.to_world(canvas_rect.center()).to_pos2();
        let Some(current) = grid::page_index_at_or_near(
            center,
            self.columns,
            self.settings.settings().page_gap,
            page_count,
        ) else {
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
            let world_rect = grid::page_rect(i, self.columns, self.settings.settings().page_gap);
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

    fn handle_selection_input(
        &mut self,
        canvas_rect: egui::Rect,
        response: &Response,
        postit_rects: &[egui::Rect],
    ) {
        if response.drag_started_by(PointerButton::Primary) {
            let pointer = response.interact_pointer_pos();
            if pointer.is_some_and(|pos| postit_rects.iter().any(|rect| rect.contains(pos))) {
                self.selection_drag = false;
                return;
            }
            let hit = pointer.and_then(|pos| self.loc_at(canvas_rect, pos));
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
        let outcome =
            dialogs::dialog_ui(ctx, dialog, page_count, &self.search_hits, &self.bookmarks);
        match outcome {
            Outcome::Keep => {}
            Outcome::Close => {
                if matches!(dialog, Dialog::Search { .. }) {
                    self.search_hits.clear();
                    self.search_query.clear();
                    self.search_index = 0;
                }
                self.dialog = None;
            }
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
            Outcome::PostitSaved { page, index, text } => {
                self.dialog = None;
                let note = Postit { page, text };
                if let Some(index) = index {
                    if let Some(existing) = self.postits.get_mut(index) {
                        *existing = note;
                    }
                } else {
                    self.postits.push(note);
                }
                self.save_postits();
            }
            Outcome::PostitDeleted(index) => {
                self.dialog = None;
                if index < self.postits.len() {
                    self.postits.remove(index);
                    self.save_postits();
                }
            }
            Outcome::BookmarkOpen(page) => {
                self.dialog = None;
                self.jump_to_page(page, canvas_rect);
            }
            Outcome::BookmarkDeleted(page) => {
                self.bookmarks.retain(|&saved| saved != page);
                self.save_bookmarks();
            }
            Outcome::Settings(changed) => {
                let prev = self.settings.settings();
                let current_page = (changed.page_gap != prev.page_gap)
                    .then(|| self.current_page(canvas_rect))
                    .flatten();
                self.settings.update(|s| *s = changed);
                if changed.theme != prev.theme {
                    apply_theme_pref(ctx, changed.theme);
                }
                if changed.ui_scale != prev.ui_scale {
                    ctx.set_zoom_factor(changed.ui_scale);
                }
                if changed.margin != prev.margin
                    || changed.page_mode != prev.page_mode
                    || changed.font_size != prev.font_size
                {
                    self.rebuild_ready_book(ctx);
                }
                if let Some(page) = current_page {
                    let page_count = match &self.phase {
                        Phase::Ready(book) => book.pages.len(),
                        _ => 0,
                    };
                    if page_count > 0 {
                        self.center_page(page.min(page_count), canvas_rect);
                    }
                }
            }
        }
    }

    fn save_postits(&mut self) {
        let Some(book) = self.current_book.as_deref() else {
            return;
        };
        self.postit_store.set(book, self.postits.clone());
        self.postit_store.save();
    }

    fn save_bookmarks(&mut self) {
        let Some(book) = self.current_book.as_deref() else {
            return;
        };
        self.bookmark_store.set(book, self.bookmarks.clone());
        self.bookmark_store.save();
    }

    fn toggle_bookmark(&mut self, page: usize) {
        if let Some(index) = self.bookmarks.iter().position(|&saved| saved == page) {
            self.bookmarks.remove(index);
        } else {
            self.bookmarks.push(page);
            self.bookmarks.sort_unstable();
        }
        self.save_bookmarks();
    }

    fn paint_canvas(&mut self, ui: &mut egui::Ui) {
        self.poll_image_decode(ui.ctx());
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
        if std::mem::take(&mut self.pending_postit)
            && let Some(page) = self.current_page(canvas_rect)
        {
            self.dialog = Some(Dialog::Postit {
                page,
                index: None,
                text: String::new(),
            });
        }
        if std::mem::take(&mut self.pending_bookmark)
            && let Some(page) = self.current_page(canvas_rect)
        {
            self.toggle_bookmark(page);
        }

        let painter = ui.painter().with_clip_rect(canvas_rect);
        painter.rect_filled(canvas_rect, 0.0, theme::canvas_bg(ui.ctx().theme()));
        let mut postit_rects = Vec::new();

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
            Phase::Loading { progress, .. } => {
                let center = canvas_rect.center();
                let fraction = progress.fraction();
                let bar_rect = egui::Rect::from_center_size(
                    center + egui::vec2(0.0, 6.0),
                    egui::vec2(320.0, 12.0),
                );
                let visuals = ui.ctx().style_of(ui.ctx().theme()).visuals.clone();
                painter.text(
                    center - egui::vec2(0.0, 28.0),
                    egui::Align2::CENTER_CENTER,
                    "Loading…",
                    egui::FontId::proportional(18.0),
                    theme::loading_color(ui.ctx().theme()),
                );
                painter.rect_filled(bar_rect, 6.0, visuals.widgets.noninteractive.bg_fill);
                if fraction > 0.0 {
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            bar_rect.min,
                            egui::vec2(bar_rect.width() * fraction, bar_rect.height()),
                        ),
                        6.0,
                        theme::loading_color(ui.ctx().theme()),
                    );
                }
                painter.text(
                    center + egui::vec2(0.0, 30.0),
                    egui::Align2::CENTER_CENTER,
                    format!(
                        "{} · {}%",
                        progress.label(),
                        (fraction * 100.0).round() as u32
                    ),
                    egui::FontId::proportional(13.0),
                    theme::hint_color(ui.ctx().theme()),
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
                    let world_rect = grid::page_rect(i, self.columns, settings.page_gap);
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
                        textures: &book.textures,
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
                    if self.bookmarks.contains(&(i + 1)) {
                        let icon = egui::Rect::from_min_size(
                            screen_rect.min + egui::vec2(12.0, 12.0),
                            egui::vec2(14.0, 19.0),
                        );
                        let color = ctx.style_of(ctx.theme()).visuals.text_color();
                        painter.add(egui::Shape::convex_polygon(
                            vec![
                                icon.left_top(),
                                icon.right_top(),
                                icon.right_bottom(),
                                egui::pos2(icon.center().x, icon.bottom() - 5.0),
                                icon.left_bottom(),
                            ],
                            color,
                            egui::Stroke::NONE,
                        ));
                    }
                }

                let style = ctx.style_of(ctx.theme());
                let visuals = &style.visuals;
                let note_fill = visuals.window_fill;
                let note_text = visuals.text_color();
                let note_stroke = visuals.window_stroke();
                let mut stack_y: HashMap<usize, f32> = HashMap::new();
                for (index, note) in self.postits.iter().enumerate() {
                    let Some(page_index) = note.page.checked_sub(1) else {
                        continue;
                    };
                    if page_index >= book.pages.len() {
                        continue;
                    }
                    let page_rect = grid::page_rect(page_index, self.columns, settings.page_gap);
                    let zoom = self.view.zoom.max(0.01);
                    let galley = ctx.fonts_mut(|fonts| {
                        fonts.layout(
                            note.text.clone(),
                            crate::render::fonts::book(14.0 * zoom),
                            note_text,
                            144.0 * zoom,
                        )
                    });
                    let height = (galley.size().y / zoom + 16.0).clamp(36.0, 240.0);
                    let y = stack_y.entry(note.page).or_insert(page_rect.min.y + 8.0);
                    let world_rect = egui::Rect::from_min_size(
                        egui::pos2(page_rect.max.x + 8.0, *y),
                        egui::vec2(160.0, height),
                    );
                    *y += height + 8.0;
                    let screen_rect = self.view.to_screen_rect(world_rect);
                    if !canvas_rect.intersects(screen_rect) {
                        continue;
                    }
                    let clipped_rect = screen_rect.intersect(canvas_rect);
                    let response = ui.interact(
                        clipped_rect,
                        ui.id().with(("postit", index)),
                        egui::Sense::click(),
                    );
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    let clicked = response.clicked();
                    response.on_hover_ui(|ui| {
                        ui.label(
                            egui::RichText::new(note.text.clone())
                                .font(crate::render::fonts::book(14.0)),
                        );
                    });
                    painter.rect_filled(screen_rect, 4.0, note_fill);
                    painter.rect_stroke(screen_rect, 4.0, note_stroke, egui::StrokeKind::Inside);
                    painter
                        .with_clip_rect(screen_rect.shrink(8.0 * zoom))
                        .galley(
                            screen_rect.min + egui::vec2(8.0 * zoom, 8.0 * zoom),
                            galley,
                            note_text,
                        );
                    postit_rects.push(screen_rect);
                    if clicked {
                        self.dialog = Some(Dialog::Postit {
                            page: note.page,
                            index: Some(index),
                            text: note.text.clone(),
                        });
                    }
                }
            }
        }

        self.handle_selection_input(canvas_rect, &response, &postit_rects);

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
