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

pub fn word_at(page_index: usize, page: &Page, laid: &[LaidBlock], loc: Loc) -> Option<Selection> {
    let item = page.items.get(loc.item)?;
    let block = laid.get(item.block)?;
    let LaidKind::Text { galley, .. } = &block.kind else {
        return None;
    };
    let chars: Vec<char> = galley.text().chars().collect();
    if chars.is_empty() {
        return None;
    }
    let cursor = loc.char.min(chars.len().saturating_sub(1));
    let word_char = |ch: char| ch.is_alphanumeric() || ch == '_';
    if !word_char(chars[cursor]) {
        return None;
    }
    let mut start = cursor;
    let mut end = cursor + 1;
    while start > 0 && word_char(chars[start - 1]) {
        start -= 1;
    }
    while end < chars.len() && word_char(chars[end]) {
        end += 1;
    }
    Some(Selection {
        page: page_index,
        anchor: Loc {
            item: loc.item,
            char: start,
        },
        cursor: Loc {
            item: loc.item,
            char: end,
        },
    })
}

pub fn move_cursor(page: &Page, laid: &[LaidBlock], mut loc: Loc, forward: bool) -> Loc {
    if forward {
        if let Some(item) = page.items.get(loc.item)
            && let Some(block) = laid.get(item.block)
            && let LaidKind::Text { galley, .. } = &block.kind
        {
            let len = galley.text().chars().count();
            if loc.char < len {
                loc.char += 1;
                return loc;
            }
        }
        for item_index in loc.item.saturating_add(1)..page.items.len() {
            let Some(item) = page.items.get(item_index) else {
                continue;
            };
            let Some(block) = laid.get(item.block) else {
                continue;
            };
            if let LaidKind::Text { .. } = &block.kind {
                return Loc {
                    item: item_index,
                    char: 0,
                };
            }
        }
    } else if loc.char > 0 {
        loc.char -= 1;
        return loc;
    } else {
        for item_index in (0..loc.item).rev() {
            let Some(item) = page.items.get(item_index) else {
                continue;
            };
            let Some(block) = laid.get(item.block) else {
                continue;
            };
            if let LaidKind::Text { galley, .. } = &block.kind {
                return Loc {
                    item: item_index,
                    char: galley.text().chars().count(),
                };
            }
        }
    }
    loc
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

/// Selection range as `(start, end)` char offsets into `page.plain_text`.
pub fn selection_offsets(page: &Page, sel: &Selection) -> Option<(usize, usize)> {
    let (a, b) = sel.range();
    let to_plain = |loc: &Loc| -> Option<usize> {
        let item = page.items.get(loc.item)?;
        let seg = page.segments.iter().find(|s| s.block == item.block)?;
        Some(seg.start + loc.char)
    };
    Some((to_plain(&a)?, to_plain(&b)?))
}

/// Expand the current text range to both boundaries of its containing sentence.
pub fn expand_to_sentence(page: &Page, laid: &[LaidBlock], sel: &Selection) -> Option<Selection> {
    let (start, end) = selection_offsets(page, sel)?;
    let chars: Vec<char> = page.plain_text.chars().collect();
    let (visible_start, visible_end) = visible_range(page, laid)?;
    let (paragraph_start, paragraph_end) =
        paragraph_bounds(&chars, start, end, visible_start, visible_end);
    let sentence_start = chars[paragraph_start..start.min(paragraph_end)]
        .iter()
        .rposition(|ch| is_sentence_end(*ch))
        .map_or(paragraph_start, |index| paragraph_start + index + 1);
    let sentence_end = next_sentence_end(&chars, end.min(paragraph_end), paragraph_end);
    selection_at_offsets(
        page,
        sel.page,
        trim_leading_space(&chars, sentence_start, start),
        sentence_end,
    )
}

/// Grow a sentence selection by one neighboring sentence on each side.
pub fn expand_to_adjacent_sentence(
    page: &Page,
    laid: &[LaidBlock],
    sel: &Selection,
) -> Option<Selection> {
    let (start, end) = selection_offsets(page, sel)?;
    let chars: Vec<char> = page.plain_text.chars().collect();
    let (visible_start, visible_end) = visible_range(page, laid)?;
    let (paragraph_start, paragraph_end) =
        paragraph_bounds(&chars, start, end, visible_start, visible_end);

    let previous_markers: Vec<usize> = chars[paragraph_start..start.min(paragraph_end)]
        .iter()
        .enumerate()
        .filter_map(|(index, ch)| is_sentence_end(*ch).then_some(paragraph_start + index))
        .collect();
    let next_start = if previous_markers.len() >= 2 {
        previous_markers[previous_markers.len() - 2] + 1
    } else {
        paragraph_start
    };
    let next_end = next_sentence_end(&chars, end.min(paragraph_end), paragraph_end);

    selection_at_offsets(
        page,
        sel.page,
        trim_leading_space(&chars, next_start, start),
        next_end,
    )
}

fn paragraph_bounds(
    chars: &[char],
    start: usize,
    end: usize,
    visible_start: usize,
    visible_end: usize,
) -> (usize, usize) {
    let start = start.clamp(visible_start, visible_end.min(chars.len()));
    let end = end.clamp(start, visible_end.min(chars.len()));
    let paragraph_start = chars[visible_start.min(start)..start]
        .iter()
        .rposition(|ch| *ch == '\n')
        .map_or(visible_start, |index| visible_start + index + 1);
    let paragraph_end = chars[end..visible_end.min(chars.len())]
        .iter()
        .position(|ch| *ch == '\n')
        .map_or(visible_end.min(chars.len()), |index| end + index);
    (paragraph_start, paragraph_end)
}

fn trim_leading_space(chars: &[char], mut start: usize, end: usize) -> usize {
    while start < end
        && chars
            .get(start)
            .is_some_and(|ch| ch.is_whitespace() || is_sentence_closer(*ch))
    {
        start += 1;
    }
    start
}

fn next_sentence_end(chars: &[char], start: usize, limit: usize) -> usize {
    let Some(marker) = chars[start..limit]
        .iter()
        .position(|ch| is_sentence_end(*ch))
    else {
        return limit;
    };
    let mut end = start + marker + 1;
    while end < limit && is_sentence_closer(chars[end]) {
        end += 1;
    }
    end
}

fn is_sentence_end(ch: char) -> bool {
    matches!(ch, '.' | '!' | '?')
}

fn is_sentence_closer(ch: char) -> bool {
    matches!(ch, '"' | '\'' | ')' | ']' | '}' | '”' | '’')
}

fn selection_at_offsets(
    page: &Page,
    page_index: usize,
    start: usize,
    end: usize,
) -> Option<Selection> {
    Some(Selection {
        page: page_index,
        anchor: loc_at_offset(page, start)?,
        cursor: loc_at_offset(page, end)?,
    })
}

fn visible_range(page: &Page, laid: &[LaidBlock]) -> Option<(usize, usize)> {
    let mut start = usize::MAX;
    let mut end = 0;
    for item in &page.items {
        let Some(segment) = page
            .segments
            .iter()
            .find(|segment| segment.block == item.block)
        else {
            continue;
        };
        let Some(block) = laid.get(item.block) else {
            continue;
        };
        let LaidKind::Text { galley, .. } = &block.kind else {
            continue;
        };
        let row_offset = |row: usize| {
            galley.rows[..row.min(galley.rows.len())]
                .iter()
                .map(|row| row.char_count_including_newline().0)
                .sum::<usize>()
        };
        start = start.min(segment.start + row_offset(item.row_start));
        end = end.max((segment.start + row_offset(item.row_end)).min(segment.end));
    }
    (start != usize::MAX).then_some((start, end))
}

fn loc_at_offset(page: &Page, offset: usize) -> Option<Loc> {
    for (item_index, item) in page.items.iter().enumerate() {
        let Some(segment) = page
            .segments
            .iter()
            .find(|segment| segment.block == item.block)
        else {
            continue;
        };
        if (segment.start..=segment.end).contains(&offset) {
            return Some(Loc {
                item: item_index,
                char: offset.saturating_sub(segment.start),
            });
        }
    }
    None
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

    #[test]
    fn selection_offsets_map_locs_to_plain_text() {
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
        let sel = Selection {
            page: 0,
            anchor: Loc { item: 0, char: 1 },
            cursor: Loc { item: 1, char: 3 },
        };
        assert_eq!(selection_offsets(&page, &sel), Some((1, 9)));
        let missing = Selection {
            page: 0,
            anchor: Loc { item: 9, char: 0 },
            cursor: Loc { item: 9, char: 1 },
        };
        assert_eq!(selection_offsets(&page, &missing), None);
    }

    #[test]
    fn shift_up_selects_the_sentence_then_grows_to_its_paragraph() {
        let text = "Zero. First. Second, word here. Third. Fourth!\nNext paragraph.";
        let ctx = eframe::egui::Context::default();
        crate::render::fonts::install_for_tests(&ctx);
        let blocks = [crate::book::blocks::Block::Para(vec![
            crate::book::blocks::Span {
                text: text.to_owned(),
                ..Default::default()
            },
        ])];
        let mut laid = Vec::new();
        let mut pages = Vec::new();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            laid = crate::render::layout::layout_blocks(
                ui.ctx(),
                &blocks,
                &[],
                &Default::default(),
                16.0,
                2000.0,
                1200.0,
                &crate::render::theme::PageColors::PAPER,
                crate::persist::Justify::Left,
            );
            let metrics: Vec<_> = laid.iter().map(|block| block.metrics()).collect();
            pages = crate::book::paginate::paginate(&metrics, 1200.0, false);
            crate::render::layout::attach_page_texts(&mut pages, &laid);
        });
        output.textures_delta.clear();
        let page = &pages[0];
        let start = text.find("word").unwrap();
        let selection = Selection {
            page: 0,
            anchor: Loc {
                item: 0,
                char: start,
            },
            cursor: Loc {
                item: 0,
                char: start + "beta".len(),
            },
        };

        let sentence = expand_to_sentence(page, &laid, &selection).unwrap();
        let extract = |selection: &Selection| {
            let (start, end) = selection_offsets(page, selection).unwrap();
            page.plain_text
                .chars()
                .skip(start)
                .take(end - start)
                .collect::<String>()
        };
        assert_eq!(extract(&sentence), "Second, word here.");

        let wider = expand_to_adjacent_sentence(page, &laid, &sentence).unwrap();
        assert_eq!(extract(&wider), "First. Second, word here. Third.");

        let paragraph = expand_to_adjacent_sentence(page, &laid, &wider).unwrap();
        assert_eq!(
            extract(&paragraph),
            "Zero. First. Second, word here. Third. Fourth!"
        );
    }

    #[test]
    fn double_click_word_selection_uses_character_offsets() {
        let ctx = eframe::egui::Context::default();
        crate::render::fonts::install_for_tests(&ctx);
        let blocks = [crate::book::blocks::Block::Para(vec![
            crate::book::blocks::Span {
                text: "One café word".to_owned(),
                ..Default::default()
            },
        ])];
        let mut laid = Vec::new();
        let mut pages = Vec::new();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            laid = crate::render::layout::layout_blocks(
                ui.ctx(),
                &blocks,
                &[],
                &Default::default(),
                16.0,
                300.0,
                600.0,
                &crate::render::theme::PageColors::PAPER,
                crate::persist::Justify::Left,
            );
            let metrics: Vec<_> = laid.iter().map(|block| block.metrics()).collect();
            pages = crate::book::paginate::paginate(&metrics, 600.0, false);
            crate::render::layout::attach_page_texts(&mut pages, &laid);
        });
        output.textures_delta.clear();

        let selection = word_at(0, &pages[0], &laid, Loc { item: 0, char: 6 }).unwrap();
        assert_eq!(
            selection.range(),
            (Loc { item: 0, char: 4 }, Loc { item: 0, char: 8 })
        );
    }
}
