use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui::{
    self, Color32, Context, FontId, TextureHandle, TextureOptions, Vec2, text::LayoutJob,
};

use crate::book::blocks::{Block, Span};
use crate::book::paginate::{self, BlockMetrics};
use crate::canvas::grid::PAGE_SIZE;
use crate::render::fonts;
use crate::render::theme::PageColors;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margins {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Margins {
    pub const fn uniform(value: f32) -> Self {
        Self {
            left: value,
            right: value,
            top: value,
            bottom: value,
        }
    }
}

pub const DEFAULT_MARGINS: Margins = Margins::uniform(48.0);

pub const RULE_HEIGHT: f32 = 2.0;

pub fn content_width(margins: &Margins) -> f32 {
    PAGE_SIZE.x - margins.left - margins.right
}

pub fn content_height(margins: &Margins) -> f32 {
    PAGE_SIZE.y - margins.top - margins.bottom
}

pub enum LaidKind {
    Text {
        galley: Arc<egui::text::Galley>,
        job: Arc<LayoutJob>,
        indent: f32,
        row_heights: Vec<f32>,
    },
    Image {
        texture: Arc<TextureHandle>,
        size: Vec2,
    },
    Rule,
}

pub struct LaidBlock {
    pub space_above: f32,
    pub kind: LaidKind,
}

impl LaidBlock {
    pub fn metrics(&self) -> BlockMetrics {
        match &self.kind {
            LaidKind::Text { row_heights, .. } => BlockMetrics {
                space_above: self.space_above,
                height: row_heights.iter().sum(),
                row_heights: row_heights.clone(),
            },
            LaidKind::Image { size, .. } => BlockMetrics {
                space_above: self.space_above,
                height: size.y,
                row_heights: Vec::new(),
            },
            LaidKind::Rule => BlockMetrics {
                space_above: self.space_above,
                height: RULE_HEIGHT,
                row_heights: Vec::new(),
            },
        }
    }
}

const BASE_BODY_SIZE: f32 = 14.0;
const HEADING_LEADING: f32 = 1.25;

fn heading_size(level: u8) -> f32 {
    match level {
        1 => 28.0,
        2 => 22.0,
        3 => 18.0,
        4 => 15.0,
        5 => 13.5,
        _ => 12.5,
    }
}

struct FlatBlock {
    space_above: f32,
    indent: f32,
    flush: bool,
    kind: FlatKind,
}

enum FlatKind {
    Para { spans: Vec<Span> },
    Heading { level: u8, spans: Vec<Span> },
    Image { path: String },
    Rule,
}

fn flatten(blocks: &[Block], indent: f32, out: &mut Vec<FlatBlock>) {
    let mut flush = true;
    for block in blocks {
        match block {
            Block::Heading { level, spans } => {
                out.push(FlatBlock {
                    space_above: if *level <= 2 { 20.0 } else { 14.0 },
                    indent,
                    flush: false,
                    kind: FlatKind::Heading {
                        level: *level,
                        spans: spans.clone(),
                    },
                });
                flush = true;
            }
            Block::Para(spans) => {
                out.push(FlatBlock {
                    space_above: 0.0,
                    indent,
                    flush,
                    kind: FlatKind::Para {
                        spans: spans.clone(),
                    },
                });
                flush = false;
            }
            Block::Quote(inner) => flatten(inner, indent + 16.0, out),
            Block::List { ordered, items } => {
                for (i, item) in items.iter().enumerate() {
                    let prefix = if *ordered {
                        format!("{}. ", i + 1)
                    } else {
                        "• ".to_string()
                    };
                    let start = out.len();
                    flatten(item, indent + 18.0, out);
                    if let Some(fb) = out.get_mut(start)
                        && let FlatKind::Para { spans } = &mut fb.kind
                    {
                        spans.insert(
                            0,
                            Span {
                                text: prefix,
                                ..Default::default()
                            },
                        );
                    }
                }
                flush = false;
            }
            Block::Image { path, alt: _ } => {
                out.push(FlatBlock {
                    space_above: 10.0,
                    indent,
                    flush: false,
                    kind: FlatKind::Image { path: path.clone() },
                });
                flush = true;
            }
            Block::Rule => {
                out.push(FlatBlock {
                    space_above: 12.0,
                    indent,
                    flush: false,
                    kind: FlatKind::Rule,
                });
                flush = true;
            }
        }
    }
}

fn push_spans(
    job: &mut LayoutJob,
    spans: &[Span],
    size: f32,
    color: Color32,
    code_bg: Color32,
    line_height: f32,
) {
    for span in spans {
        let mut format = egui::TextFormat {
            font_id: fonts::book(size),
            color,
            line_height: Some(line_height),
            ..Default::default()
        };
        if span.code {
            format.font_id = FontId::monospace(size * 0.9);
            format.background = code_bg;
        } else if span.bold {
            format.font_id = fonts::bold(size);
        }
        format.italics = span.italic;
        job.append(&span.text, 0.0, format);
    }
}

fn row_heights(galley: &egui::text::Galley) -> Vec<f32> {
    let rows = &galley.rows;
    if rows.is_empty() {
        return Vec::new();
    }
    let total = galley.size().y;
    let mut heights = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let next = rows
            .get(i + 1)
            .map(|r| r.pos.y)
            .unwrap_or_else(|| rows[0].pos.y + total);
        heights.push((next - row.pos.y).max(1.0));
    }
    heights
}

pub fn layout_blocks(
    ctx: &Context,
    blocks: &[Block],
    images: &HashMap<String, Arc<TextureHandle>>,
    body_size: f32,
    content_width: f32,
    content_height: f32,
    colors: &PageColors,
) -> Vec<LaidBlock> {
    let mut flat = Vec::new();
    flatten(blocks, 0.0, &mut flat);

    let mut out = Vec::with_capacity(flat.len());
    for fb in flat {
        match fb.kind {
            FlatKind::Para { spans } => {
                let mut job = LayoutJob::default();
                job.wrap.max_width = content_width - fb.indent;
                job.justify = true;
                push_spans(
                    &mut job,
                    &spans,
                    body_size,
                    colors.body,
                    colors.code_bg,
                    body_size * 1.4,
                );
                if !fb.flush
                    && let Some(first) = job.sections.first_mut()
                {
                    first.leading_space += body_size * 2.0;
                }
                if job.text.is_empty() {
                    continue;
                }
                let job = Arc::new(job);
                let galley = ctx.fonts_mut(|f| f.layout_job((*job).clone()));
                let heights = row_heights(&galley);
                if heights.is_empty() {
                    continue;
                }
                out.push(LaidBlock {
                    space_above: fb.space_above,
                    kind: LaidKind::Text {
                        galley,
                        job,
                        indent: fb.indent,
                        row_heights: heights,
                    },
                });
            }
            FlatKind::Heading { level, spans } => {
                let mut job = LayoutJob::default();
                job.wrap.max_width = content_width - fb.indent;
                if level == 1 {
                    job.halign = egui::Align::Center;
                }
                let size = heading_size(level) * body_size / BASE_BODY_SIZE;
                push_spans(
                    &mut job,
                    &spans,
                    size,
                    colors.heading,
                    colors.code_bg,
                    size * HEADING_LEADING,
                );
                if job.text.is_empty() {
                    continue;
                }
                let job = Arc::new(job);
                let galley = ctx.fonts_mut(|f| f.layout_job((*job).clone()));
                let heights = row_heights(&galley);
                if heights.is_empty() {
                    continue;
                }
                out.push(LaidBlock {
                    space_above: fb.space_above,
                    kind: LaidKind::Text {
                        galley,
                        job,
                        indent: fb.indent,
                        row_heights: heights,
                    },
                });
            }
            FlatKind::Image { path } => {
                let Some(texture) = images.get(&path) else {
                    continue;
                };
                let size = texture.size_vec2();
                let max_w = content_width - fb.indent;
                let mut scale = (max_w / size.x).min(1.0);
                if size.y * scale > content_height {
                    scale = content_height / size.y;
                }
                out.push(LaidBlock {
                    space_above: fb.space_above,
                    kind: LaidKind::Image {
                        texture: Arc::clone(texture),
                        size: size * scale,
                    },
                });
            }
            FlatKind::Rule => out.push(LaidBlock {
                space_above: fb.space_above,
                kind: LaidKind::Rule,
            }),
        }
    }

    out
}

/// Fill each page's `plain_text` and block segments from the laid galleys.
/// Char offsets; blocks joined with a newline.
pub fn attach_page_texts(pages: &mut [paginate::Page], laid: &[LaidBlock]) {
    for page in pages.iter_mut() {
        let mut text = String::new();
        let mut chars = 0usize;
        let mut segments = Vec::new();
        for item in &page.items {
            let Some(block) = laid.get(item.block) else {
                continue;
            };
            let LaidKind::Text { galley, .. } = &block.kind else {
                continue;
            };
            if !text.is_empty() {
                text.push('\n');
                chars += 1;
            }
            let start = chars;
            text.push_str(galley.text());
            chars += galley.text().chars().count();
            segments.push(paginate::TextSegment {
                block: item.block,
                start,
                end: chars,
            });
        }
        page.plain_text = text;
        page.segments = segments;
    }
}

/// Rebuild a layout job at `scale` — every size-bearing field grows linearly,
/// so line breaks land where the zoom=1 layout put them (modulo float noise).
pub fn scaled_job(job: &LayoutJob, scale: f32) -> LayoutJob {
    if scale == 1.0 {
        return job.clone();
    }
    let mut out = job.clone();
    out.wrap.max_width *= scale;
    out.first_row_min_height *= scale;
    for section in &mut out.sections {
        section.leading_space *= scale;
        section.format.font_id.size *= scale;
        section.format.extra_letter_spacing *= scale;
        if let Some(line_height) = &mut section.format.line_height {
            *line_height *= scale;
        }
    }
    out
}

/// Galleys re-laid at a given zoom, keyed by (block index, zoom bits).
/// Only what the user has actually looked at gets re-laid.
#[derive(Default)]
pub struct DisplayCache {
    entries: HashMap<(usize, u32), Arc<egui::text::Galley>>,
}

const DISPLAY_CACHE_CAP: usize = 4096;

impl DisplayCache {
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns the galley to paint plus the scale its coordinates are in:
    /// `1.0` when the galley was re-laid at `zoom`, or `zoom` when falling
    /// back to the zoom=1 galley (row-count drift after re-layout).
    pub fn galley_for(
        &mut self,
        ctx: &Context,
        block: usize,
        base: &Arc<egui::text::Galley>,
        job: &LayoutJob,
        zoom: f32,
    ) -> (Arc<egui::text::Galley>, f32) {
        if zoom == 1.0 {
            return (Arc::clone(base), 1.0);
        }
        let key = (block, zoom.to_bits());
        if let Some(cached) = self.entries.get(&key) {
            return (Arc::clone(cached), 1.0);
        }
        let scaled = ctx.fonts_mut(|f| f.layout_job(scaled_job(job, zoom)));
        if scaled.rows.len() != base.rows.len() {
            return (Arc::clone(base), zoom);
        }
        if self.entries.len() >= DISPLAY_CACHE_CAP {
            self.entries.clear();
        }
        self.entries.insert(key, Arc::clone(&scaled));
        (scaled, 1.0)
    }
}

pub fn decode_image(bytes: &[u8]) -> Option<egui::ColorImage> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(
        size,
        img.as_flat_samples().samples,
    ))
}

pub fn load_texture(ctx: &Context, name: &str, image: &egui::ColorImage) -> TextureHandle {
    ctx.load_texture(name, image.clone(), TextureOptions::LINEAR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::blocks::{Block, Span};
    use crate::book::paginate;
    use crate::book::prepare::prepare;
    use std::path::Path;

    #[test]
    fn body_jobs_are_justified_with_book_leading_and_indent() {
        let ctx = Context::default();
        crate::render::fonts::install_for_tests(&ctx);

        let spans = || {
            vec![Span {
                text: "word ".repeat(30),
                ..Default::default()
            }]
        };
        let blocks = vec![
            Block::Heading {
                level: 1,
                spans: vec![Span {
                    text: "Chapter One".to_owned(),
                    ..Default::default()
                }],
            },
            Block::Para(spans()),
            Block::Para(spans()),
            Block::Heading {
                level: 2,
                spans: vec![Span {
                    text: "Section".to_owned(),
                    ..Default::default()
                }],
            },
            Block::Para(spans()),
        ];

        let margins = DEFAULT_MARGINS;
        let colors = PageColors::PAPER;
        let mut jobs: Vec<LayoutJob> = Vec::new();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let laid = layout_blocks(
                ui.ctx(),
                &blocks,
                &HashMap::new(),
                16.0,
                content_width(&margins),
                content_height(&margins),
                &colors,
            );
            for b in &laid {
                if let LaidKind::Text { job, .. } = &b.kind {
                    jobs.push((**job).clone());
                }
            }
        });
        output.textures_delta.clear();
        assert_eq!(jobs.len(), 5);

        assert_eq!(jobs[0].halign, egui::Align::Center);
        assert!(!jobs[0].justify);
        let h1_leading = jobs[0].sections[0].format.line_height.unwrap();
        assert!(
            (h1_leading - heading_size(1) * 16.0 / BASE_BODY_SIZE * HEADING_LEADING).abs() < 1e-3,
            "headings get tighter leading ratio than body"
        );

        assert!(jobs[1].justify);
        assert_eq!(jobs[1].sections[0].leading_space, 0.0);
        assert_eq!(jobs[1].sections[0].format.line_height, Some(16.0 * 1.4));
        assert_eq!(jobs[1].sections[0].format.font_id.size, 16.0);

        assert!(jobs[2].justify);
        assert_eq!(jobs[2].sections[0].leading_space, 16.0 * 2.0);

        assert_eq!(jobs[3].halign, egui::Align::LEFT);

        assert_eq!(
            jobs[4].sections[0].leading_space, 0.0,
            "flush after heading"
        );
    }

    #[test]
    fn lays_out_and_paginates_real_book() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/books/pg1342.epub");
        let raw = prepare(&path).expect("prepare pg1342");
        assert!(!raw.blocks.is_empty());

        let ctx = Context::default();
        crate::render::fonts::install_for_tests(&ctx);

        let margins = DEFAULT_MARGINS;
        let width = content_width(&margins);
        let height = content_height(&margins);
        let colors = PageColors::PAPER;

        let mut laid = Vec::new();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            laid = layout_blocks(
                ui.ctx(),
                &raw.blocks,
                &HashMap::new(),
                16.0,
                width,
                height,
                &colors,
            );
        });
        output.textures_delta.clear();
        assert!(!laid.is_empty());

        let metrics: Vec<_> = laid.iter().map(|b| b.metrics()).collect();
        let pages = paginate::paginate(&metrics, height, raw.cover.is_some());
        assert!(pages.len() > 50);

        for page in &pages {
            if page.kind == paginate::PageKind::Cover {
                continue;
            }
            for item in &page.items {
                assert!(item.y < height + 1.0);
                assert!(item.row_start < item.row_end);
            }
        }
    }

    #[test]
    fn page_colors_do_not_affect_row_breaks() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/books/pg1342.epub");
        let raw = prepare(&path).expect("prepare pg1342");

        let ctx = Context::default();
        crate::render::fonts::install_for_tests(&ctx);

        let margins = DEFAULT_MARGINS;
        let width = content_width(&margins);
        let height = content_height(&margins);

        let mut paper_rows = Vec::new();
        let mut dark_rows = Vec::new();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let paper = layout_blocks(
                ui.ctx(),
                &raw.blocks,
                &HashMap::new(),
                16.0,
                width,
                height,
                &PageColors::PAPER,
            );
            let dark = layout_blocks(
                ui.ctx(),
                &raw.blocks,
                &HashMap::new(),
                16.0,
                width,
                height,
                &PageColors::DARK_PAGES,
            );
            for block in &paper {
                if let LaidKind::Text { row_heights, .. } = &block.kind {
                    paper_rows.push(row_heights.len());
                }
            }
            for block in &dark {
                if let LaidKind::Text { row_heights, .. } = &block.kind {
                    dark_rows.push(row_heights.len());
                }
            }
        });
        output.textures_delta.clear();

        assert!(!paper_rows.is_empty());
        assert_eq!(paper_rows, dark_rows);
    }

    #[test]
    fn scaled_job_multiplies_all_sizes() {
        let mut job = LayoutJob::default();
        job.wrap.max_width = 500.0;
        job.first_row_min_height = 10.0;
        job.append(
            "hello",
            6.0,
            egui::TextFormat {
                font_id: FontId::proportional(12.0),
                ..Default::default()
            },
        );

        let scaled = scaled_job(&job, 2.0);
        assert_eq!(scaled.wrap.max_width, 1000.0);
        assert_eq!(scaled.first_row_min_height, 20.0);
        assert_eq!(scaled.sections[0].leading_space, 12.0);
        assert_eq!(scaled.sections[0].format.font_id.size, 24.0);
        assert_eq!(scaled.text, job.text);
    }

    #[test]
    fn display_cache_relays_at_zoom_with_same_row_count() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/books/pg1342.epub");
        let raw = prepare(&path).expect("prepare pg1342");
        let ctx = Context::default();
        crate::render::fonts::install_for_tests(&ctx);

        let mut cache = DisplayCache::default();
        let mut checked = 0usize;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let margins = DEFAULT_MARGINS;
            let laid = layout_blocks(
                ui.ctx(),
                &raw.blocks,
                &HashMap::new(),
                16.0,
                content_width(&margins),
                content_height(&margins),
                &PageColors::PAPER,
            );
            for (bi, block) in laid.iter().enumerate() {
                if let LaidKind::Text { galley, job, .. } = &block.kind {
                    let (g, scale) = cache.galley_for(ui.ctx(), bi, galley, job, 1.5);
                    assert_eq!(scale, 1.0, "expected re-laid galley at zoom 1.5");
                    assert_eq!(g.rows.len(), galley.rows.len());
                    assert!(g.size().y > galley.size().y);
                    checked += 1;
                    if checked >= 20 {
                        break;
                    }
                }
            }
        });
        output.textures_delta.clear();
        assert_eq!(checked, 20);
    }

    #[test]
    fn search_hits_map_to_piece_ranges_on_real_book() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/books/pg1342.epub");
        let raw = prepare(&path).expect("prepare pg1342");

        let ctx = Context::default();
        crate::render::fonts::install_for_tests(&ctx);

        let margins = DEFAULT_MARGINS;
        let colors = PageColors::PAPER;
        let mut pages = Vec::new();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let laid = layout_blocks(
                ui.ctx(),
                &raw.blocks,
                &HashMap::new(),
                16.0,
                content_width(&margins),
                content_height(&margins),
                &colors,
            );
            let metrics: Vec<_> = laid.iter().map(|b| b.metrics()).collect();
            pages = paginate::paginate(&metrics, content_height(&margins), raw.cover.is_some());
            attach_page_texts(&mut pages, &laid);

            let hits = crate::features::search::search(&pages, "the");
            assert!(!hits.is_empty(), "pg1342 must contain 'the'");
            let mut mapped = 0usize;
            for hit in &hits {
                let ranges =
                    crate::features::selection::piece_ranges(&pages[hit.page], hit.start, hit.end);
                assert!(
                    !ranges.is_empty(),
                    "hit at page {} chars {}..{} maps to no piece (segments: {:?})",
                    hit.page,
                    hit.start,
                    hit.end,
                    pages[hit.page].segments
                );
                mapped += ranges.len();
            }
            assert_eq!(mapped, hits.len());
        });
        output.textures_delta.clear();
    }
}
