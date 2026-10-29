use eframe::egui::{self, Rect, Vec2};

use crate::book::paginate::{Page, PageKind};
use crate::render::layout::{LaidBlock, LaidKind, Margins};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Loc {
    pub item: usize,
    pub char: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub page: usize,
    pub anchor: Loc,
    pub cursor: Loc,
}

impl Selection {
    pub fn single(page: usize, loc: Loc) -> Self {
        Self {
            page,
            anchor: loc,
            cursor: loc,
        }
    }

    /// Anchor/cursor in reading order.
    pub fn range(&self) -> (Loc, Loc) {
        if self.anchor <= self.cursor {
            (self.anchor, self.cursor)
        } else {
            (self.cursor, self.anchor)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.cursor
    }
}

/// Page-local position (world units, origin at page top-left, zoom = 1) → piece char cursor.
pub fn hit_test(page: &Page, laid: &[LaidBlock], margins: Margins, page_pos: Vec2) -> Option<Loc> {
    if page.kind == PageKind::Cover {
        return None;
    }
    for (ii, item) in page.items.iter().enumerate() {
        let block = laid.get(item.block)?;
        let LaidKind::Text { galley, indent, .. } = &block.kind else {
            continue;
        };
        let row_first = galley.rows.get(item.row_start)?;
        let row_last = galley.rows.get(item.row_end.checked_sub(1)?)?;
        let origin = Vec2::new(margins.left + *indent, margins.top + item.y);
        let galley_pos = origin - row_first.pos.to_vec2();
        let local = page_pos - galley_pos;
        let top = row_first.min_y();
        let bottom = row_last.max_y();
        let left = -4.0;
        let right = galley.rect.width() + 4.0;
        if local.y >= top && local.y <= bottom && local.x >= left && local.x <= right {
            let cursor = galley.cursor_from_pos(local);
            return Some(Loc {
                item: ii,
                char: cursor.index.0,
            });
        }
    }
    None
}

/// Selected text of one page: pieces `[start.item..=end.item]`, newline between pieces.
pub fn copy_text(page: &Page, laid: &[LaidBlock], sel: &Selection) -> String {
    let (a, b) = sel.range();
    let mut out = String::new();
    for (ii, item) in page.items.iter().enumerate() {
        if ii < a.item || ii > b.item {
            continue;
        }
        let Some(block) = laid.get(item.block) else {
            continue;
        };
        let LaidKind::Text { galley, .. } = &block.kind else {
            continue;
        };
        let start = if ii == a.item { a.char } else { 0 };
        let end = if ii == b.item { b.char } else { usize::MAX };
        push_char_slice(&mut out, galley.text(), start, end);
    }
    out
}

fn push_char_slice(out: &mut String, text: &str, start: usize, end: usize) {
    if !out.is_empty() {
        out.push('\n');
    }
    let chars: Vec<char> = text.chars().collect();
    let start = start.min(chars.len());
    let end = end.min(chars.len()).max(start);
    out.extend(chars[start..end].iter());
}

/// Highlight rects (galley space) for char range `[a, b)` restricted to the
/// piece's visible row band `[row_start, row_end)`. One rect per covered row.
pub fn span_rects(
    galley: &egui::text::Galley,
    row_start: usize,
    row_end: usize,
    a: usize,
    b: usize,
) -> Vec<Rect> {
    if a >= b {
        return Vec::new();
    }
    let end = row_end.min(galley.rows.len());
    if row_start >= end {
        return Vec::new();
    }

    let mut ra = galley.pos_from_cursor(egui::text::CCursor::new(a));
    let mut rb = galley.pos_from_cursor(egui::text::CCursor::new(b));

    let row_of = |y: f32| -> usize {
        for r in row_start..end {
            let row = &galley.rows[r];
            if y >= row.min_y() && y < row.max_y() {
                return r;
            }
        }
        if y < galley.rows[row_start].min_y() {
            row_start
        } else {
            end - 1
        }
    };

    let mut ar = row_of(ra.top());
    let mut br = row_of(rb.top());
    if (br, rb.left()) < (ar, ra.left()) {
        std::mem::swap(&mut ra, &mut rb);
        std::mem::swap(&mut ar, &mut br);
    }

    let mut rects = Vec::new();
    for r in ar..=br {
        let row = &galley.rows[r];
        let row_left = row.pos.x;
        let row_right = row_left + row.row.size.x;
        let (x0, x1) = if ar == br {
            (ra.left().min(rb.left()), ra.left().max(rb.left()))
        } else if r == ar {
            (ra.left(), row_right)
        } else if r == br {
            (row_left, rb.left())
        } else {
            (row_left, row_right)
        };
        let x0 = x0.max(row_left);
        let x1 = x1.min(row_right);
        if x1 > x0 {
            rects.push(Rect::from_min_max(
                egui::pos2(x0, row.min_y()),
                egui::pos2(x1, row.max_y()),
            ));
        }
    }
    rects
}

/// Map a page-text char range to per-piece char ranges: `(item index, a, b)`.
pub fn piece_ranges(page: &Page, a: usize, b: usize) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    if a >= b {
        return out;
    }
    for (ii, item) in page.items.iter().enumerate() {
        let Some(seg) = page
            .segments
            .iter()
            .find(|s| s.block == item.block && a < s.end && b > s.start)
        else {
            continue;
        };
        let start = seg.start.max(a);
        let end = seg.end.min(b);
        if start < end {
            out.push((ii, start - seg.start, end - seg.start));
        }
    }
    out
}

/// Item spans `(item, char a, char b)` covering the selection's pieces.
pub fn selection_spans(page: &Page, sel: &Selection) -> Vec<(usize, usize, usize)> {
    let (a, b) = sel.range();
    let last = page.items.len().saturating_sub(1);
    if a.item > last {
        return Vec::new();
    }
    let mut out = Vec::new();
    for ii in a.item..=b.item.min(last) {
        let start = if ii == a.item { a.char } else { 0 };
        let end = if ii == b.item { b.char } else { usize::MAX };
        if start < end {
            out.push((ii, start, end));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::paginate::{Page, PageItem, PageKind, TextSegment};

    fn empty_page() -> Page {
        Page {
            kind: PageKind::Content,
            items: Vec::new(),
            plain_text: String::new(),
            segments: Vec::new(),
        }
    }

    #[test]
    fn range_orders_anchor_and_cursor() {
        let a = Loc { item: 2, char: 5 };
        let b = Loc { item: 1, char: 9 };
        let sel = Selection {
            page: 0,
            anchor: a,
            cursor: b,
        };
        assert_eq!(sel.range(), (b, a));
        assert!(!sel.is_empty());
        let empty = Selection::single(0, a);
        assert!(empty.is_empty());
    }

    #[test]
    fn selection_spans_cover_items_between_anchor_and_cursor() {
        let page = Page {
            kind: PageKind::Content,
            items: vec![
                PageItem {
                    block: 0,
                    row_start: 0,
                    row_end: 1,
                    y: 0.0,
                },
                PageItem {
                    block: 1,
                    row_start: 0,
                    row_end: 1,
                    y: 10.0,
                },
                PageItem {
                    block: 2,
                    row_start: 0,
                    row_end: 1,
                    y: 20.0,
                },
            ],
            plain_text: String::new(),
            segments: Vec::new(),
        };
        let sel = Selection {
            page: 0,
            anchor: Loc { item: 0, char: 2 },
            cursor: Loc { item: 2, char: 3 },
        };
        assert_eq!(
            selection_spans(&page, &sel),
            vec![(0, 2, usize::MAX), (1, 0, usize::MAX), (2, 0, 3)]
        );
        let empty = Selection::single(0, Loc { item: 5, char: 0 });
        assert!(selection_spans(&page, &empty).is_empty());
    }

    #[test]
    fn hit_test_misses_cover_and_empty_page() {
        let cover = Page {
            kind: PageKind::Cover,
            ..empty_page()
        };
        let margins = Margins::uniform(48.0);
        assert!(hit_test(&cover, &[], margins, Vec2::new(100.0, 100.0)).is_none());
        assert!(hit_test(&empty_page(), &[], margins, Vec2::new(100.0, 100.0)).is_none());
    }

    #[test]
    fn span_rects_same_row_yields_single_rect() {
        let ctx = eframe::egui::Context::default();
        crate::render::fonts::install_for_tests(&ctx);

        let mut galley = None;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let mut job = egui::text::LayoutJob::default();
            job.wrap.max_width = 200.0;
            job.append(
                "hello world of typography",
                0.0,
                egui::TextFormat::default(),
            );
            galley = Some(ui.ctx().fonts_mut(|f| f.layout_job(job)));
        });
        output.textures_delta.clear();
        let galley = galley.unwrap();

        // "world" on the (single) row
        let rects = span_rects(&galley, 0, galley.rows.len(), 6, 11);
        assert_eq!(rects.len(), 1, "same-row range must yield one rect");
        assert!(rects[0].width() > 0.0);
        assert!(rects[0].height() > 0.0);
    }

    #[test]
    fn span_rects_multi_row_covers_each_row() {
        let ctx = eframe::egui::Context::default();
        crate::render::fonts::install_for_tests(&ctx);

        let mut galley = None;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let mut job = egui::text::LayoutJob::default();
            job.wrap.max_width = 60.0;
            job.append(
                "word ".repeat(30).as_str(),
                0.0,
                egui::TextFormat::default(),
            );
            galley = Some(ui.ctx().fonts_mut(|f| f.layout_job(job)));
        });
        output.textures_delta.clear();
        let galley = galley.unwrap();
        assert!(galley.rows.len() >= 3, "need a wrapping galley");

        let rects = span_rects(&galley, 0, galley.rows.len(), 0, galley.text().len());
        assert!(rects.len() >= 2, "full range must cover several rows");
        for r in &rects {
            assert!(r.width() > 0.0 && r.height() > 0.0);
        }

        // Empty and inverted ranges produce nothing.
        assert!(span_rects(&galley, 0, galley.rows.len(), 5, 5).is_empty());
        assert!(span_rects(&galley, 0, galley.rows.len(), 9, 3).is_empty());
    }

    #[test]
    fn piece_ranges_map_page_offsets_to_items() {
        let page = Page {
            kind: PageKind::Content,
            items: vec![
                PageItem {
                    block: 3,
                    row_start: 0,
                    row_end: 2,
                    y: 0.0,
                },
                PageItem {
                    block: 4,
                    row_start: 0,
                    row_end: 2,
                    y: 50.0,
                },
            ],
            plain_text: "hello\nworld".to_owned(),
            segments: vec![
                TextSegment {
                    block: 3,
                    start: 0,
                    end: 5,
                },
                TextSegment {
                    block: 4,
                    start: 6,
                    end: 11,
                },
            ],
        };
        assert_eq!(piece_ranges(&page, 3, 8), vec![(0, 3, 5), (1, 0, 2)]);
        assert!(piece_ranges(&page, 4, 4).is_empty());
    }
}
