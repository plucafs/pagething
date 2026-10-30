#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKind {
    Cover,
    Content,
}

#[derive(Debug, Clone)]
pub struct PageItem {
    pub block: usize,
    pub row_start: usize,
    pub row_end: usize,
    pub y: f32,
}

/// A block's text inside its page's `plain_text`, in char offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSegment {
    pub block: usize,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct Page {
    pub kind: PageKind,
    pub items: Vec<PageItem>,
    pub plain_text: String,
    pub segments: Vec<TextSegment>,
}

#[derive(Debug, Clone)]
pub struct BlockMetrics {
    pub space_above: f32,
    pub page_break_before: bool,
    pub height: f32,
    pub row_heights: Vec<f32>,
}

impl BlockMetrics {
    fn segment_heights(&self) -> &[f32] {
        if self.row_heights.is_empty() {
            std::slice::from_ref(&self.height)
        } else {
            &self.row_heights
        }
    }
}

pub fn paginate(blocks: &[BlockMetrics], content_height: f32, has_cover: bool) -> Vec<Page> {
    let mut pages = Vec::new();
    if has_cover {
        pages.push(Page {
            kind: PageKind::Cover,
            items: Vec::new(),
            plain_text: String::new(),
            segments: Vec::new(),
        });
    }

    let mut items: Vec<PageItem> = Vec::new();
    let mut y = 0.0f32;

    let finish_page = |items: &mut Vec<PageItem>, pages: &mut Vec<Page>| {
        pages.push(Page {
            kind: PageKind::Content,
            items: std::mem::take(items),
            plain_text: String::new(),
            segments: Vec::new(),
        });
    };

    for (bi, block) in blocks.iter().enumerate() {
        let segs = block.segment_heights();
        if segs.is_empty() {
            continue;
        }
        if block.page_break_before && !items.is_empty() {
            finish_page(&mut items, &mut pages);
            y = 0.0;
        }
        let mut seg = 0usize;
        let mut first = true;

        while seg < segs.len() {
            let gap = if first && y > 0.0 {
                block.space_above
            } else {
                0.0
            };
            let avail = content_height - y - gap;

            if avail <= 0.0 {
                finish_page(&mut items, &mut pages);
                y = 0.0;
                continue;
            }

            let mut take = 0usize;
            let mut used = 0.0f32;
            while seg + take < segs.len() && used + segs[seg + take] <= avail {
                used += segs[seg + take];
                take += 1;
            }

            let forced = take == 0;
            if take == 0 {
                if items.is_empty() && y <= 0.0 {
                    take = 1;
                    used = segs[seg];
                } else {
                    finish_page(&mut items, &mut pages);
                    y = 0.0;
                    continue;
                }
            }

            if !forced {
                let after = seg + take;
                if segs.len() - after == 1 && used + segs[after] <= avail {
                    used += segs[after];
                    take += 1;
                }
                if take == 1 && segs.len() - seg > 1 && !items.is_empty() {
                    finish_page(&mut items, &mut pages);
                    y = 0.0;
                    continue;
                }
            }

            items.push(PageItem {
                block: bi,
                row_start: seg,
                row_end: seg + take,
                y: y + gap,
            });
            y += gap + used;
            seg += take;
            first = false;

            if seg < segs.len() {
                finish_page(&mut items, &mut pages);
                y = 0.0;
            }
        }
    }

    if !items.is_empty() || pages.is_empty() {
        finish_page(&mut items, &mut pages);
    }

    pages
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(space: f32, height: f32) -> BlockMetrics {
        BlockMetrics {
            space_above: space,
            page_break_before: false,
            height,
            row_heights: Vec::new(),
        }
    }

    fn rows(space: f32, heights: Vec<f32>) -> BlockMetrics {
        BlockMetrics {
            space_above: space,
            page_break_before: false,
            height: heights.iter().sum(),
            row_heights: heights,
        }
    }

    #[test]
    fn packs_small_blocks_on_one_page() {
        let blocks = [solid(0.0, 100.0), solid(10.0, 100.0)];
        let pages = paginate(&blocks, 500.0, false);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].items.len(), 2);
        assert_eq!(pages[0].items[1].y, 110.0);
    }

    #[test]
    fn splits_text_block_across_pages() {
        let blocks = [rows(0.0, vec![100.0; 10])];
        let pages = paginate(&blocks, 450.0, false);
        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0].items[0].row_end, 4);
        assert_eq!(pages[1].items[0].row_start, 4);
        assert_eq!(pages[1].items[0].row_end, 8);
        assert_eq!(pages[2].items[0].row_start, 8);
    }

    #[test]
    fn moves_solid_block_whole_to_next_page() {
        let blocks = [solid(0.0, 400.0), solid(10.0, 300.0)];
        let pages = paginate(&blocks, 500.0, false);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].items.len(), 1);
        assert_eq!(pages[1].items[0].block, 1);
        assert_eq!(pages[1].items[0].y, 0.0);
    }

    #[test]
    fn drops_space_above_at_page_top() {
        let blocks = [solid(0.0, 480.0), solid(40.0, 100.0)];
        let pages = paginate(&blocks, 500.0, false);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[1].items[0].y, 0.0);
    }

    #[test]
    fn chapter_start_moves_to_new_page_without_empty_pages() {
        let mut chapter = solid(0.0, 100.0);
        chapter.page_break_before = true;
        let blocks = [solid(0.0, 300.0), chapter, solid(0.0, 100.0)];

        let pages = paginate(&blocks, 500.0, false);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].items.len(), 1);
        assert_eq!(pages[1].items[0].block, 1);
        assert_eq!(pages[1].items[0].y, 0.0);
        assert_eq!(pages[1].items[1].block, 2);

        let mut first = solid(0.0, 100.0);
        first.page_break_before = true;
        let pages = paginate(&[first], 500.0, true);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[1].items.len(), 1);
    }

    #[test]
    fn cover_page_prepended() {
        let blocks = [solid(0.0, 10.0)];
        let pages = paginate(&blocks, 500.0, true);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].kind, PageKind::Cover);
        assert_eq!(pages[1].kind, PageKind::Content);
    }

    #[test]
    fn oversized_solid_fills_forced_page() {
        let blocks = [solid(0.0, 800.0)];
        let pages = paginate(&blocks, 500.0, false);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].items[0].row_end, 1);
    }

    #[test]
    fn widow_pulls_final_row_onto_previous_page_when_it_fits() {
        let blocks = [rows(0.0, vec![80.0, 80.0, 80.0, 80.0, 70.0])];
        let pages = paginate(&blocks, 400.0, false);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].items[0].row_end, 5);
    }

    #[test]
    fn widow_left_when_final_row_does_not_fit() {
        let blocks = [rows(0.0, vec![100.0; 3])];
        let pages = paginate(&blocks, 250.0, false);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].items[0].row_end, 2);
        assert_eq!(pages[1].items[0].row_start, 2);
        assert_eq!(pages[1].items[0].row_end, 3);
    }

    #[test]
    fn orphan_pushes_single_row_to_next_page() {
        let blocks = [solid(0.0, 300.0), rows(0.0, vec![150.0; 3])];
        let pages = paginate(&blocks, 500.0, false);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].items.len(), 1);
        assert_eq!(pages[0].items[0].block, 0);
        assert_eq!(pages[1].items.len(), 1);
        assert_eq!(pages[1].items[0].block, 1);
        assert_eq!(pages[1].items[0].row_end - pages[1].items[0].row_start, 3);
    }

    #[test]
    fn forced_single_row_still_places_oversized_segment() {
        let blocks = [rows(0.0, vec![600.0, 100.0])];
        let pages = paginate(&blocks, 500.0, false);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].items[0].row_end, 1);
        assert_eq!(pages[1].items[0].row_start, 1);
    }

    #[test]
    fn empty_blocks_with_cover() {
        let pages = paginate(&[], 500.0, true);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].kind, PageKind::Cover);
    }
}
