use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use eframe::egui::{self, Color32, Key, Modifiers, ViewportCommand};

use crate::book::paginate;
use crate::book::prepare::{self, PreparedBook};
use crate::canvas::{self, View, grid};
use crate::features::dialogs::{self, Dialog, Outcome};
use crate::persist::{SettingsStore, ViewState, ViewStore};
use crate::render::layout::{self, DisplayCache, LaidBlock};
use crate::render::page_view::{self, PageView};

const MAX_RECENTS: usize = 10;

struct LoadedBook {
    title: Option<String>,
    cover: Option<Arc<egui::TextureHandle>>,
    laid: Vec<LaidBlock>,
    pages: Vec<paginate::Page>,
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
}

impl PagethingApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::render::fonts::install(&cc.egui_ctx);
        let settings = SettingsStore::load();
        Self {
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
        }
    }

    fn open_path(&mut self, path: PathBuf) {
        self.stash_view();
        self.display_cache.clear();
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

    fn build_book(ctx: &egui::Context, raw: PreparedBook) -> LoadedBook {
        let mut textures = HashMap::new();
        for (name, image) in raw.images {
            let handle = layout::load_texture(ctx, &name, &image);
            textures.insert(name, Arc::new(handle));
        }
        let cover = raw
            .cover
            .map(|image| Arc::new(layout::load_texture(ctx, "cover", &image)));

        let laid = layout::layout_blocks(ctx, &raw.blocks, &textures, layout::content_width());
        let metrics: Vec<_> = laid.iter().map(|b| b.metrics()).collect();
        let pages = paginate::paginate(&metrics, layout::content_height(), cover.is_some());

        LoadedBook {
            title: raw.title,
            cover,
            laid,
            pages,
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
                    let book = Self::build_book(ctx, raw);
                    self.phase = Phase::Ready(book);
                }
                Err(error) => self.phase = Phase::Empty { error: Some(error) },
            }
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::O)) {
            self.want_open = true;
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
                    if ui.button("Quit").clicked() {
                        ui.send_viewport_cmd(ViewportCommand::Close);
                    }
                });

                if let Phase::Ready(book) = &self.phase
                    && let Some(title) = &book.title
                {
                    ui.label(egui::RichText::new(title).weak());
                }
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

    fn apply_dialog(&mut self, canvas_rect: egui::Rect, ctx: &egui::Context) {
        let Some(dialog) = &mut self.dialog else {
            return;
        };
        let page_count = match &self.phase {
            Phase::Ready(book) => book.pages.len(),
            _ => 0,
        };
        let outcome = dialogs::dialog_ui(ctx, dialog, page_count);
        match outcome {
            Outcome::Keep => {}
            Outcome::Close => self.dialog = None,
            Outcome::Jumped(page) => {
                self.dialog = None;
                let world_rect = grid::page_rect(page - 1, self.columns, grid::GAP);
                let world_center = world_rect.center();
                self.view.pan = canvas_rect.center() - world_center * self.view.zoom;
            }
            Outcome::Columns(columns) => {
                self.dialog = None;
                self.columns = columns;
                self.settings.set_columns(columns);
            }
        }
    }

    fn paint_canvas(&mut self, ui: &mut egui::Ui) {
        let canvas_rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(canvas_rect, egui::Sense::click_and_drag());
        self.view.handle_input(ui.ctx(), canvas_rect, &response);

        let painter = ui.painter().with_clip_rect(canvas_rect);
        painter.rect_filled(canvas_rect, 0.0, canvas::CANVAS_BG);

        match &self.phase {
            Phase::Empty { error } => {
                let (message, color) = match error {
                    Some(e) => (
                        format!("Could not open book: {e}"),
                        Color32::from_rgb(220, 100, 100),
                    ),
                    None => ("Ctrl+O to open an EPUB".to_owned(), Color32::from_gray(160)),
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
                    Color32::WHITE,
                );
            }
            Phase::Ready(book) => {
                let ctx = ui.ctx().clone();
                let font_size = ctx.fonts(|f| f.font_image_size());
                let font_uv = egui::Vec2::new(
                    1.0 / font_size[0].max(1) as f32,
                    1.0 / font_size[1].max(1) as f32,
                );
                for (i, page) in book.pages.iter().enumerate() {
                    let world_rect = grid::page_rect(i, self.columns, grid::GAP);
                    let screen_rect = self.view.to_screen_rect(world_rect);
                    if !canvas_rect.intersects(screen_rect) {
                        continue;
                    }
                    let pv = PageView {
                        page,
                        laid: &book.laid,
                        cover: book.cover.as_ref(),
                        number: i + 1,
                        font_uv,
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
        ctx.request_repaint();
    }
}

impl eframe::App for PagethingApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_shortcuts(&ctx);
        self.menu_bar(ui);
        self.run_file_dialog();
        self.poll_loading(&ctx);
        self.paint_canvas(ui);
    }

    fn on_exit(&mut self) {
        self.stash_view();
    }
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
