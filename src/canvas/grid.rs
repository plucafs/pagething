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

pub fn page_index_at_or_near(
    point: Pos2,
    columns: usize,
    gap: f32,
    page_count: usize,
) -> Option<usize> {
    (0..page_count).min_by(|&a, &b| {
        page_distance_sq(point, page_rect(a, columns, gap))
            .total_cmp(&page_distance_sq(point, page_rect(b, columns, gap)))
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
        assert_eq!(
            page_index_at_or_near(page_rect(1, 4, GAP).center(), 4, GAP, 3),
            Some(1)
        );
        assert_eq!(
            page_index_at_or_near(Pos2::new(0.0, 0.0), 4, GAP, 3),
            Some(0)
        );
        assert_eq!(
            page_index_at_or_near(
                Pos2::new(
                    CANVAS_MARGIN.x + PAGE_SIZE.x + GAP * 0.25,
                    CANVAS_MARGIN.y + PAGE_SIZE.y * 0.5,
                ),
                4,
                GAP,
                3,
            ),
            Some(0)
        );
        assert_eq!(page_index_at_or_near(Pos2::ZERO, 4, GAP, 0), None);
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
