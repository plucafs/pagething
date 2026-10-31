use eframe::egui::{Pos2, Rect, Vec2};

use crate::persist::PageRatio;

pub const DEFAULT_COLUMNS: usize = 5;
pub const GAP: f32 = 128.0;

pub const PAGE_WIDTH: f32 = 600.0;
pub const CANVAS_MARGIN: Vec2 = Vec2::splat(48.0);

pub fn page_size(ratio: PageRatio) -> Vec2 {
    Vec2::new(PAGE_WIDTH, PAGE_WIDTH * ratio.height_over_width())
}

pub fn page_rect(index: usize, columns: usize, gap: f32, size: Vec2) -> Rect {
    let columns = columns.max(1);
    let col = index % columns;
    let row = index / columns;
    let min = Pos2::new(
        CANVAS_MARGIN.x + col as f32 * (size.x + gap),
        CANVAS_MARGIN.y + row as f32 * (size.y + gap),
    );
    Rect::from_min_size(min, size)
}

pub fn page_index_at_or_near(
    point: Pos2,
    columns: usize,
    gap: f32,
    page_count: usize,
    size: Vec2,
) -> Option<usize> {
    (0..page_count).min_by(|&a, &b| {
        page_distance_sq(point, page_rect(a, columns, gap, size))
            .total_cmp(&page_distance_sq(point, page_rect(b, columns, gap, size)))
    })
}

pub fn adjacent_page_index(current: usize, page_count: usize, forward: bool) -> Option<usize> {
    if page_count == 0 {
        return None;
    }

    Some(if forward {
        if current + 1 == page_count {
            0
        } else {
            current + 1
        }
    } else if current == 0 {
        page_count - 1
    } else {
        current - 1
    })
}

fn page_distance_sq(point: Pos2, rect: Rect) -> f32 {
    let nearest = Pos2::new(
        point.x.clamp(rect.min.x, rect.max.x),
        point.y.clamp(rect.min.y, rect.max.y),
    );
    point.distance_sq(nearest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_page_under_or_nearest_to_view_center() {
        let size = page_size(PageRatio::default());
        assert_eq!(
            page_index_at_or_near(page_rect(1, 4, GAP, size).center(), 4, GAP, 3, size),
            Some(1)
        );
        assert_eq!(
            page_index_at_or_near(Pos2::new(0.0, 0.0), 4, GAP, 3, size),
            Some(0)
        );
        assert_eq!(
            page_index_at_or_near(
                Pos2::new(
                    CANVAS_MARGIN.x + size.x + GAP * 0.25,
                    CANVAS_MARGIN.y + size.y * 0.5,
                ),
                4,
                GAP,
                3,
                size,
            ),
            Some(0)
        );
        assert_eq!(page_index_at_or_near(Pos2::ZERO, 4, GAP, 0, size), None);
    }

    #[test]
    fn page_size_uses_fixed_width_and_selected_ratio() {
        assert_eq!(page_size(PageRatio::Square), Vec2::new(600.0, 600.0));
        assert_eq!(page_size(PageRatio::ThreeFour), Vec2::new(600.0, 800.0));
        assert_eq!(page_size(PageRatio::TwoThree), Vec2::new(600.0, 900.0));
        assert_eq!(page_size(PageRatio::FiveEight), Vec2::new(600.0, 960.0));
        assert!((page_size(PageRatio::A5).y - 600.0 * 2.0_f32.sqrt()).abs() < 1e-4);
    }

    #[test]
    fn adjacent_pages_wrap_in_both_directions() {
        assert_eq!(adjacent_page_index(0, 5, false), Some(4));
        assert_eq!(adjacent_page_index(4, 5, true), Some(0));
        assert_eq!(adjacent_page_index(2, 5, false), Some(1));
        assert_eq!(adjacent_page_index(2, 5, true), Some(3));
        assert_eq!(adjacent_page_index(0, 0, true), None);
    }
}
