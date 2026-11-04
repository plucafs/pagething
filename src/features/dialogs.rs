use std::path::PathBuf;

use eframe::egui::{self, Align2, CursorIcon, Key, Modifiers, Sense};

use crate::features::recents::TitleLoader;
use crate::features::search::Hit;
use crate::persist::{
    MAX_FONT_SIZE, MAX_MARGIN, MAX_PAGE_GAP, MAX_UI_SCALE, MIN_FONT_SIZE, MIN_MARGIN, MIN_PAGE_GAP,
    MIN_UI_SCALE, PageMode, PageRatio, Reply, Settings, ThemePref, TitleStore, fmt_ts,
};

pub const MAX_COLUMNS: usize = 16;

#[derive(Debug, Clone)]
pub enum Dialog {
    Jump {
        input: String,
    },
    Columns {
        input: String,
    },
    Settings {
        draft: Settings,
    },
    Search {
        input: String,
    },
    Postit {
        page: usize,
        index: Option<usize>,
        text: String,
        created_at: u64,
        updated_at: u64,
        replies: Vec<Reply>,
        reply_draft: String,
    },
    Bookmarks,
    Recents {
        editing: Option<usize>,
        draft: String,
    },
    Shortcuts,
    Help,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Keep,
    Close,
    Jumped(usize),
    Columns(usize),
    Settings(Settings),
    Search(String),
    SearchPrev,
    SearchJump(usize),
    PostitSaved {
        page: usize,
        index: Option<usize>,
        text: String,
    },
    PostitDeleted(usize),
    PostitReply {
        index: usize,
        text: String,
    },
    PostitReplyDeleted {
        index: usize,
        reply_index: usize,
    },
    BookmarkOpen(usize),
    BookmarkDeleted(usize),
    OpenRecent {
        path: PathBuf,
        new_window: bool,
    },
    RenameRecent {
        path: PathBuf,
        title: String,
    },
    RemoveRecent(PathBuf),
}

pub fn parse_jump(input: &str, page_count: usize) -> Option<usize> {
    if page_count == 0 {
        return None;
    }
    let n: usize = input.trim().parse().ok()?;
    Some(n.clamp(1, page_count))
}

pub fn parse_columns(input: &str) -> Option<usize> {
    let n: usize = input.trim().parse().ok()?;
    Some(n.clamp(1, MAX_COLUMNS))
}

#[allow(clippy::too_many_arguments)]
pub fn dialog_ui(
    ctx: &egui::Context,
    dialog: &mut Dialog,
    page_count: usize,
    hits: &[Hit],
    bookmarks: &[usize],
    recents: &[PathBuf],
    titles: &TitleLoader,
    custom_titles: &TitleStore,
) -> Outcome {
    let mut outcome = Outcome::Keep;

    let title = match dialog {
        Dialog::Jump { .. } => "Jump to page",
        Dialog::Columns { .. } => "Columns",
        Dialog::Settings { .. } => "Settings",
        Dialog::Search { .. } => "Search",
        Dialog::Postit { index, .. } => {
            if index.is_some() {
                "Edit post-it"
            } else {
                "New post-it"
            }
        }
        Dialog::Bookmarks => "Bookmarks",
        Dialog::Recents { .. } => "Recent books",
        Dialog::Shortcuts => "Shortcuts",
        Dialog::Help => "About pagething",
    };
    let anchor = match dialog {
        Dialog::Search { .. } | Dialog::Bookmarks | Dialog::Postit { .. } => {
            (Align2::RIGHT_TOP, egui::vec2(-12.0, 40.0))
        }
        _ => (Align2::CENTER_CENTER, egui::Vec2::ZERO),
    };

    let mut open = true;
    egui::Window::new(title)
        .anchor(anchor.0, anchor.1)
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ctx, |ui| match dialog {
            Dialog::Jump { input } => {
                let edit = ui.add(
                    egui::TextEdit::singleline(input)
                        .hint_text(format!("1–{page_count}"))
                        .desired_width(160.0),
                );
                edit.request_focus();

                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    outcome = match parse_jump(input, page_count) {
                        Some(page) => Outcome::Jumped(page),
                        None => Outcome::Close,
                    };
                } else if ui.input(|i| i.key_pressed(Key::Escape)) {
                    outcome = Outcome::Close;
                }
            }
            Dialog::Columns { input } => {
                let edit = ui.add(
                    egui::TextEdit::singleline(input)
                        .hint_text(format!("1–{MAX_COLUMNS}"))
                        .desired_width(160.0),
                );
                edit.request_focus();

                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    outcome = match parse_columns(input) {
                        Some(columns) => Outcome::Columns(columns),
                        None => Outcome::Close,
                    };
                } else if ui.input(|i| i.key_pressed(Key::Escape)) {
                    outcome = Outcome::Close;
                }
            }
            Dialog::Settings { draft } => {
                let mut changed = false;

                ui.label("Margins");
                let slider = ui.add(
                    egui::Slider::new(&mut draft.margin, MIN_MARGIN..=MAX_MARGIN)
                        .step_by(2.0)
                        .fixed_decimals(0),
                );
                if slider.drag_stopped() || (slider.changed() && !slider.dragged()) {
                    changed = true;
                }

                ui.separator();
                ui.label("Page gap");
                let gap = ui.add(
                    egui::Slider::new(&mut draft.page_gap, MIN_PAGE_GAP..=MAX_PAGE_GAP)
                        .step_by(4.0)
                        .fixed_decimals(0)
                        .suffix(" px"),
                );
                if gap.drag_stopped() || (gap.changed() && !gap.dragged()) {
                    changed = true;
                }

                ui.separator();
                ui.label("Font size");
                let font_size = ui.add(
                    egui::Slider::new(&mut draft.font_size, MIN_FONT_SIZE..=MAX_FONT_SIZE)
                        .step_by(1.0)
                        .fixed_decimals(0)
                        .suffix(" pt"),
                );
                if font_size.drag_stopped() || (font_size.changed() && !font_size.dragged()) {
                    changed = true;
                }

                ui.separator();
                ui.label("UI scale");
                let scale = ui.add(
                    egui::Slider::new(&mut draft.ui_scale, MIN_UI_SCALE..=MAX_UI_SCALE)
                        .step_by(0.1)
                        .fixed_decimals(2),
                );
                if scale.changed() {
                    changed = true;
                }

                ui.separator();
                ui.label("Theme");
                ui.horizontal(|ui| {
                    for (pref, label) in [
                        (ThemePref::Light, "Light"),
                        (ThemePref::Dark, "Dark"),
                        (ThemePref::System, "System"),
                    ] {
                        if ui.selectable_value(&mut draft.theme, pref, label).clicked() {
                            changed = true;
                        }
                    }
                });

                ui.label("Page theme");
                ui.horizontal(|ui| {
                    for (mode, label) in [
                        (PageMode::Paper, "Light"),
                        (PageMode::DarkPages, "Dark"),
                        (PageMode::Sepia, "Sepia"),
                    ] {
                        if ui
                            .selectable_value(&mut draft.page_mode, mode, label)
                            .clicked()
                        {
                            changed = true;
                        }
                    }
                });

                ui.separator();
                ui.label("Page size");
                ui.horizontal(|ui| {
                    for (ratio, label) in [
                        (PageRatio::Square, "1:1"),
                        (PageRatio::ThreeFour, "3:4"),
                        (PageRatio::A5, "A5"),
                        (PageRatio::TwoThree, "2:3"),
                        (PageRatio::FiveEight, "5:8"),
                    ] {
                        if ui.selectable_value(&mut draft.page_ratio, ratio, label).clicked() {
                            changed = true;
                        }
                    }
                });

                ui.separator();
                if ui.checkbox(&mut draft.toolbar, "Show action bar").changed() {
                    changed = true;
                }

                if changed {
                    outcome = Outcome::Settings(*draft);
                }
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    outcome = Outcome::Close;
                }
            }
            Dialog::Search { input } => {
                let edit = ui.add(
                    egui::TextEdit::singleline(input)
                        .hint_text("Search book")
                        .desired_width(240.0),
                );
                edit.request_focus();

                let enter = ui.input(|i| i.key_pressed(Key::Enter));
                let shift = ui.input(|i| i.modifiers.shift);
                if enter && !input.trim().is_empty() {
                    outcome = if shift {
                        Outcome::SearchPrev
                    } else {
                        Outcome::Search(input.clone())
                    };
                } else if ui.input(|i| i.key_pressed(Key::Escape)) {
                    outcome = Outcome::Close;
                }

                if !hits.is_empty() {
                    ui.separator();
                    ui.label(format!("{} matches — Enter/Shift+Enter cycles", hits.len()));
                    egui::ScrollArea::vertical()
                        .max_height(240.0)
                        .show(ui, |ui| {
                            for (index, hit) in hits.iter().enumerate() {
                                let label = format!("p.{}  {}", hit.page + 1, hit.snippet);
                                if ui.button(label).clicked() {
                                    outcome = Outcome::SearchJump(index);
                                }
                            }
                        });
                }
            }
            Dialog::Postit {
                page,
                index,
                text,
                created_at,
                updated_at,
                replies,
                reply_draft,
            } => {
                ui.label(format!("Page {page}"));
                if *created_at > 0 {
                    ui.label(
                        egui::RichText::new(format!(
                            "Created {} · Updated {}",
                            fmt_ts(*created_at),
                            fmt_ts(*updated_at)
                        ))
                        .weak()
                        .small(),
                    );
                }
                let body = ui.add(
                    egui::TextEdit::multiline(text)
                        .font(crate::render::fonts::book(16.0))
                        .desired_width(320.0)
                        .desired_rows(6)
                        .hint_text("Write a note…"),
                );
                // Focus the body only when the dialog opens (nothing focused yet),
                // so clicking the reply field isn't stolen every frame.
                if ui.ctx().memory(|m| m.focused()).is_none() {
                    body.request_focus();
                }

                if ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.ctrl) {
                    outcome = save_postit(*page, *index, text);
                }
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        outcome = save_postit(*page, *index, text);
                    }
                    if let Some(index) = index
                        && ui.button("Delete").clicked()
                    {
                        outcome = Outcome::PostitDeleted(*index);
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = Outcome::Close;
                    }
                });

                if !replies.is_empty() || index.is_some() {
                    ui.separator();
                    ui.label(egui::RichText::new("Replies").strong().small());
                    egui::ScrollArea::vertical()
                        .max_height(140.0)
                        .show(ui, |ui| {
                            for (reply_index, reply) in replies.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    if let Some(index) = index
                                        && ui
                                            .small_button("x")
                                            .on_hover_text("Delete reply")
                                            .clicked()
                                    {
                                        outcome = Outcome::PostitReplyDeleted {
                                            index: *index,
                                            reply_index,
                                        };
                                    }
                                    ui.label(
                                        egui::RichText::new(fmt_ts(reply.created_at))
                                            .weak()
                                            .small(),
                                    );
                                    let copy_w = 48.0;
                                    let text_w =
                                        (ui.available_width() - copy_w - 8.0).max(48.0);
                                    ui.allocate_ui_with_layout(
                                        egui::vec2(text_w, ui.spacing().interact_size.y),
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |ui| {
                                            ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(reply.text.clone())
                                                        .font(
                                                            crate::render::fonts::book(14.0),
                                                        ),
                                                )
                                                .truncate()
                                                .selectable(false),
                                            );
                                        },
                                    );
                                    if ui
                                        .add_sized(
                                            [copy_w, ui.spacing().interact_size.y],
                                            egui::Button::new("Copy"),
                                        )
                                        .clicked()
                                    {
                                        ctx.copy_text(reply.text.clone());
                                    }
                                });
                            }
                        });
                    if let Some(index) = index {
                        let reply_resp = ui.add(
                            egui::TextEdit::multiline(reply_draft)
                                .font(crate::render::fonts::book(14.0))
                                .desired_width(320.0)
                                .desired_rows(2)
                                .hint_text("Reply… (Enter to send, Shift+Enter for newline)")
                                .return_key(egui::KeyboardShortcut::new(
                                    egui::Modifiers::SHIFT,
                                    Key::Enter,
                                )),
                        );
                        ui.horizontal(|ui| {
                            if ui.button("Reply").clicked()
                                && !reply_draft.trim().is_empty()
                            {
                                outcome = Outcome::PostitReply {
                                    index: *index,
                                    text: reply_draft.trim().to_owned(),
                                };
                            }
                            ui.label(
                                egui::RichText::new("Enter sends · Shift+Enter new line")
                                    .weak()
                                    .small(),
                            );
                        });
                        // Enter (without Shift) sends while the reply box has focus.
                        if reply_resp.has_focus()
                            && ui.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift)
                            && !reply_draft.trim().is_empty()
                        {
                            outcome = Outcome::PostitReply {
                                index: *index,
                                text: reply_draft.trim().to_owned(),
                            };
                        }
                    }
                }

                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    outcome = Outcome::Close;
                }
            }
            Dialog::Bookmarks => {
                let valid = bookmarks
                    .iter()
                    .copied()
                    .filter(|&page| page > 0 && page <= page_count);
                let mut any = false;
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .show(ui, |ui| {
                        for page in valid {
                            any = true;
                            ui.horizontal(|ui| {
                                if ui.button(format!("Page {page}")).clicked() {
                                    outcome = Outcome::BookmarkOpen(page);
                                }
                                if ui.small_button("x").clicked() {
                                    outcome = Outcome::BookmarkDeleted(page);
                                }
                            });
                        }
                    });
                if !any {
                    ui.label("No bookmarks yet. Press v on a page to add one.");
                }
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    outcome = Outcome::Close;
                }
            }
            Dialog::Recents { editing, draft } => {
                const LIST_WIDTH: f32 = 420.0;
                const LIST_HEIGHT: f32 = 480.0;
                const ROW_HEIGHT: f32 = 36.0;
                const TEXT_HEIGHT: f32 = 18.0;
                const EDIT_WIDTH: f32 = 24.0;
                const REMOVE_WIDTH: f32 = 20.0;
                ui.allocate_ui_with_layout(
                    egui::vec2(LIST_WIDTH, LIST_HEIGHT),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(LIST_HEIGHT)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                if recents.is_empty() {
                                    ui.label("No recent books yet. Press Ctrl+O to open one.");
                                    return;
                                }
                                for (index, path) in recents.iter().enumerate() {
                                    let display = custom_titles
                                        .get(path)
                                        .map(str::to_owned)
                                        .or_else(|| titles.title(path).map(str::to_owned))
                                        .unwrap_or_else(|| {
                                            path.file_name()
                                                .map(|n| n.to_string_lossy().into_owned())
                                                .unwrap_or_else(|| path.display().to_string())
                                        });
                                    let row_width = ui.available_width();
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(row_width, ROW_HEIGHT),
                                        Sense::click(),
                                    );
                                    if response.hovered() {
                                        ui.painter().rect_filled(
                                            rect,
                                            4.0,
                                            ui.visuals().faint_bg_color,
                                        );
                                    }
                                    let response =
                                        response.on_hover_cursor(CursorIcon::PointingHand);
                                    let edit_rect = egui::Rect::from_min_size(
                                        egui::pos2(
                                            rect.max.x
                                                - 8.0
                                                - REMOVE_WIDTH
                                                - 4.0
                                                - EDIT_WIDTH,
                                            rect.center().y - 10.0,
                                        ),
                                        egui::vec2(EDIT_WIDTH, 20.0),
                                    );
                                    let edit_button = ui
                                        .put(edit_rect, egui::Button::new("✏️").small())
                                        .on_hover_text("Rename");
                                    if edit_button.clicked() {
                                        *editing = Some(index);
                                        *draft = display.clone();
                                    }
                                    let remove_rect = egui::Rect::from_min_size(
                                        egui::pos2(rect.max.x - 8.0 - REMOVE_WIDTH, rect.center().y - 10.0),
                                        egui::vec2(REMOVE_WIDTH, 20.0),
                                    );
                                    let remove_button = ui
                                        .put(remove_rect, egui::Button::new("x").small())
                                        .on_hover_text("Remove from recent books");
                                    if remove_button.clicked() {
                                        outcome = Outcome::RemoveRecent(path.clone());
                                    }
                                    if response.clicked() {
                                        if editing.is_some() {
                                            *editing = None;
                                        } else {
                                            let new_window = ui.input(|i| {
                                                i.modifiers.ctrl || i.modifiers.command
                                            });
                                            outcome = Outcome::OpenRecent {
                                                path: path.clone(),
                                                new_window,
                                            };
                                        }
                                    }
                                    let text_rect = egui::Rect::from_min_size(
                                        rect.min + egui::vec2(
                                            8.0,
                                            (ROW_HEIGHT - TEXT_HEIGHT) / 2.0,
                                        ),
                                        egui::vec2(
                                            rect.width() - 16.0 - EDIT_WIDTH - REMOVE_WIDTH - 10.0,
                                            TEXT_HEIGHT,
                                        ),
                                    );
                                    if *editing == Some(index) {
                                        let edit = ui.put(
                                            text_rect,
                                            egui::TextEdit::singleline(draft)
                                                .desired_width(text_rect.width()),
                                        );
                                        edit.request_focus();
                                    } else {
                                        ui.put(
                                            text_rect,
                                            egui::Label::new(display)
                                                .halign(egui::Align::Min)
                                                .truncate()
                                                .selectable(false),
                                        );
                                    }
                                }
                            });
                    },
                );
                if ui.input(|i| i.key_pressed(Key::Enter))
                    && let Some(index) = *editing
                    && let Some(path) = recents.get(index)
                {
                    outcome = Outcome::RenameRecent {
                        path: path.clone(),
                        title: draft.clone(),
                    };
                    *editing = None;
                }
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    if editing.is_some() {
                        *editing = None;
                    } else {
                        outcome = Outcome::Close;
                    }
                }
            }
            Dialog::Shortcuts => {
                let keyboard = [
                    ("Ctrl+O", "Open EPUB"),
                    ("Ctrl+R", "Recent books"),
                    ("Ctrl+N", "Open a new window"),
                    ("Ctrl+Q", "Quit"),
                    ("Ctrl+G", "Go to page"),
                    ("Ctrl+L", "Set columns"),
                    ("Ctrl+F", "Search the book"),
                    ("Ctrl+,", "Open settings"),
                    ("a", "Add a post-it to the current page"),
                    ("v", "Toggle a bookmark on the current page"),
                    ("Ctrl+V", "Show bookmarks"),
                    ("Ctrl+0", "Reset view"),
                    ("Ctrl + / Ctrl =", "Zoom in on the book"),
                    ("Ctrl -", "Zoom out on the book"),
                    ("Ctrl+C", "Copy selected text"),
                    ("c", "Copy selected text"),
                    ("f", "Find selected text"),
                    ("h", "Highlight selection (adds a post-it)"),
                    ("q", "Clear text selection"),
                    ("z / Left / PgUp", "Previous page (wraps)"),
                    ("x / Right / PgDn", "Next page (wraps)"),
                    ("Home", "First page"),
                    ("End", "Last page"),
                    ("Enter", "Next search result"),
                    ("Shift+Enter", "Previous search result"),
                ];
                let mouse = [
                    ("Left-drag", "Select text on a page"),
                    ("Middle-drag", "Pan the canvas"),
                    ("Wheel", "Zoom toward the pointer"),
                    ("Two-finger scroll", "Pan the canvas"),
                ];

                egui::ScrollArea::vertical()
                    .max_height(420.0)
                    .show(ui, |ui| {
                        ui.strong("Keyboard");
                        egui::Grid::new("shortcuts_keyboard")
                            .num_columns(2)
                            .spacing([16.0, 4.0])
                            .show(ui, |ui| {
                                for (shortcut, action) in keyboard {
                                    ui.monospace(shortcut);
                                    ui.label(action);
                                    ui.end_row();
                                }
                            });
                        ui.add_space(8.0);
                        ui.strong("Mouse and trackpad");
                        egui::Grid::new("shortcuts_pointer")
                            .num_columns(2)
                            .spacing([16.0, 4.0])
                            .show(ui, |ui| {
                                for (shortcut, action) in mouse {
                                    ui.monospace(shortcut);
                                    ui.label(action);
                                    ui.end_row();
                                }
                            });
                    });
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    outcome = Outcome::Close;
                }
            }
            Dialog::Help => {
                ui.label("pagething is an infinite-canvas EPUB reader.");
                ui.label("Open a book from File → Open, then pan and zoom across its pages.");
                ui.label("Drag across text to select it. Choose About → Shortcuts for the full reference.");
                ui.separator();
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    outcome = Outcome::Close;
                }
            }
        });

    if !open {
        outcome = Outcome::Close;
    }

    outcome
}

fn save_postit(page: usize, index: Option<usize>, text: &str) -> Outcome {
    if text.trim().is_empty() {
        Outcome::Close
    } else {
        Outcome::PostitSaved {
            page,
            index,
            text: text.to_owned(),
        }
    }
}

/// True when the shortcut should not fall through to other handlers.
pub fn consume_shortcut(ctx: &egui::Context, key: Key) -> bool {
    ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_jump_valid_clamps_and_rejects() {
        assert_eq!(parse_jump("42", 100), Some(42));
        assert_eq!(parse_jump(" 7 ", 100), Some(7));
        assert_eq!(parse_jump("0", 100), Some(1));
        assert_eq!(parse_jump("99999", 100), Some(100));
        assert_eq!(parse_jump("", 100), None);
        assert_eq!(parse_jump("abc", 100), None);
        assert_eq!(parse_jump("12", 0), None);
    }

    #[test]
    fn parse_columns_clamps_and_rejects() {
        assert_eq!(parse_columns("6"), Some(6));
        assert_eq!(parse_columns("0"), Some(1));
        assert_eq!(parse_columns("99"), Some(MAX_COLUMNS));
        assert_eq!(parse_columns("x"), None);
        assert_eq!(parse_columns(""), None);
    }
}
