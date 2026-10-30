use eframe::egui::{self, Align2, Key, Modifiers};

use crate::features::search::Hit;
use crate::persist::{
    MAX_MARGIN, MAX_UI_SCALE, MIN_MARGIN, MIN_UI_SCALE, PageMode, Settings, ThemePref,
};

pub const MAX_COLUMNS: usize = 16;

#[derive(Debug, Clone)]
pub enum Dialog {
    Jump { input: String },
    Columns { input: String },
    Settings { draft: Settings },
    Search { input: String },
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

pub fn dialog_ui(
    ctx: &egui::Context,
    dialog: &mut Dialog,
    page_count: usize,
    hits: &[Hit],
) -> Outcome {
    let mut outcome = Outcome::Keep;

    let title = match dialog {
        Dialog::Jump { .. } => "Jump to page",
        Dialog::Columns { .. } => "Columns",
        Dialog::Settings { .. } => "Settings",
        Dialog::Search { .. } => "Search",
        Dialog::Shortcuts => "Shortcuts",
        Dialog::Help => "About pagething",
    };
    let anchor = match dialog {
        Dialog::Search { .. } => (Align2::RIGHT_TOP, egui::vec2(-12.0, 40.0)),
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
                    ] {
                        if ui
                            .selectable_value(&mut draft.page_mode, mode, label)
                            .clicked()
                        {
                            changed = true;
                        }
                    }
                });

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
            Dialog::Shortcuts => {
                let keyboard = [
                    ("Ctrl+O", "Open EPUB"),
                    ("Ctrl+N", "Open a new window"),
                    ("Ctrl+Q", "Quit"),
                    ("Ctrl+G", "Go to page"),
                    ("Ctrl+L", "Set columns"),
                    ("Ctrl+F", "Search the book"),
                    ("Ctrl+,", "Open settings"),
                    ("Ctrl+0", "Reset view"),
                    ("Ctrl+C", "Copy selected text"),
                    ("c", "Copy selected text"),
                    ("f", "Find selected text"),
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
