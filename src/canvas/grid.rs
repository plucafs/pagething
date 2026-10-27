use eframe::egui::{Pos2, Rect, Vec2};

pub const DEFAULT_COLUMNS: usize = 4;
pub const GAP: f32 = 24.0;

pub const PAGE_SIZE: Vec2 = Vec2::new(600.0, 900.0);
pub const CANVAS_MARGIN: Vec2 = Vec2::splat(48.0);

pub fn page_rect(index: usize, columns: usize, gap: f32) -> Rect {
    let columns = columns.max(1);
    let col = index % columns;
    let row = index / columns;
    let min = Pos2::new(
        CANVAS_MARGIN.x + col as f32 * (PAGE_SIZE.x + gap),
        CANVAS_MARGIN.y + row as f32 * (PAGE_SIZE.y + gap),
    );
    Rect::from_min_size(min, PAGE_SIZE)
}
