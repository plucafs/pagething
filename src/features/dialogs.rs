use eframe::egui::{self, Align2, Key, Modifiers};

pub const MAX_COLUMNS: usize = 16;

#[derive(Debug, Clone)]
pub enum Dialog {
    Jump { input: String },
    Columns { input: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Keep,
    Close,
    Jumped(usize),
    Columns(usize),
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

pub fn dialog_ui(ctx: &egui::Context, dialog: &mut Dialog, page_count: usize) -> Outcome {
    let mut outcome = Outcome::Keep;

    let (title, hint) = match dialog {
        Dialog::Jump { .. } => ("Jump to page", format!("1–{page_count}")),
        Dialog::Columns { .. } => ("Columns", format!("1–{MAX_COLUMNS}")),
    };

    egui::Window::new(title)
        .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            let input = match dialog {
                Dialog::Jump { input } | Dialog::Columns { input } => input,
            };
            let edit = ui.add(
                egui::TextEdit::singleline(input)
                    .hint_text(hint)
                    .desired_width(160.0),
            );
            edit.request_focus();

            if ui.input(|i| i.key_pressed(Key::Enter)) {
                outcome = match dialog {
                    Dialog::Jump { input } => match parse_jump(input, page_count) {
                        Some(page) => Outcome::Jumped(page),
                        None => Outcome::Close,
                    },
                    Dialog::Columns { input } => match parse_columns(input) {
                        Some(columns) => Outcome::Columns(columns),
                        None => Outcome::Close,
                    },
                };
            } else if ui.input(|i| i.key_pressed(Key::Escape)) {
                outcome = Outcome::Close;
            }
        });

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
