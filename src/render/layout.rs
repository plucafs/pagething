use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui::{
    self, Color32, Context, FontId, TextureHandle, TextureOptions, Vec2, text::LayoutJob,
};

use crate::book::blocks::{Block, Span};
use crate::book::paginate::BlockMetrics;
use crate::canvas::grid::PAGE_SIZE;
use crate::render::fonts;

#[derive(Debug, Clone, Copy)]
pub struct Margins {
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

pub const MARGINS: Margins = Margins {
    left: 48.0,
    right: 48.0,
    top: 48.0,
    bottom: 72.0,
};

pub const RULE_HEIGHT: f32 = 2.0;

pub fn content_width() -> f32 {
    PAGE_SIZE.x - MARGINS.left - MARGINS.right
}

pub fn content_height() -> f32 {
    PAGE_SIZE.y - MARGINS.top - MARGINS.bottom
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

const BODY_SIZE: f32 = 12.0;
const BODY_COLOR: Color32 = Color32::from_gray(40);
const QUOTE_COLOR: Color32 = Color32::from_gray(90);
const CODE_BG: Color32 = Color32::from_gray(235);

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
    quote: bool,
    kind: FlatKind,
}

enum FlatKind {
    Para { spans: Vec<Span> },
    Heading { level: u8, spans: Vec<Span> },
    Image { path: String },
    Rule,
}

fn flatten(blocks: &[Block], indent: f32, quote: bool, out: &mut Vec<FlatBlock>) {
    for block in blocks {
        match block {
            Block::Heading { level, spans } => out.push(FlatBlock {
                space_above: if *level <= 2 { 20.0 } else { 14.0 },
                indent,
                quote,
                kind: FlatKind::Heading {
                    level: *level,
                    spans: spans.clone(),
                },
            }),
            Block::Para(spans) => out.push(FlatBlock {
                space_above: 6.0,
                indent,
                quote,
                kind: FlatKind::Para {
                    spans: spans.clone(),
                },
            }),
            Block::Quote(inner) => flatten(inner, indent + 16.0, true, out),
            Block::List { ordered, items } => {
                for (i, item) in items.iter().enumerate() {
                    let prefix = if *ordered {
                        format!("{}. ", i + 1)
                    } else {
                        "• ".to_string()
                    };
                    let start = out.len();
                    flatten(item, indent + 18.0, quote, out);
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
            }
            Block::Image { path, alt: _ } => out.push(FlatBlock {
                space_above: 10.0,
                indent,
                quote,
                kind: FlatKind::Image { path: path.clone() },
            }),
            Block::Rule => out.push(FlatBlock {
                space_above: 12.0,
                indent,
                quote,
                kind: FlatKind::Rule,
            }),
        }
    }
}

fn push_spans(job: &mut LayoutJob, spans: &[Span], size: f32, color: Color32) {
    for span in spans {
        let mut format = egui::TextFormat {
            font_id: FontId::proportional(size),
            color,
            ..Default::default()
        };
        if span.code {
            format.font_id = FontId::monospace(size * 0.9);
            format.background = CODE_BG;
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
    content_width: f32,
) -> Vec<LaidBlock> {
    let mut flat = Vec::new();
    flatten(blocks, 0.0, false, &mut flat);

    let mut out = Vec::with_capacity(flat.len());
    for fb in flat {
        match fb.kind {
            FlatKind::Para { spans } => {
                let color = if fb.quote { QUOTE_COLOR } else { BODY_COLOR };
                let mut job = LayoutJob::default();
                job.wrap.max_width = content_width - fb.indent;
                push_spans(&mut job, &spans, BODY_SIZE, color);
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
                push_spans(
                    &mut job,
                    &spans,
                    heading_size(level),
                    Color32::from_gray(20),
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
                if size.y * scale > content_height() {
                    scale = content_height() / size.y;
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
    use crate::book::paginate;
    use crate::book::prepare::prepare;
    use std::path::Path;

    #[test]
    fn lays_out_and_paginates_real_book() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/books/pg1342.epub");
        let raw = prepare(&path).expect("prepare pg1342");
        assert!(!raw.blocks.is_empty());

        let ctx = Context::default();
        crate::render::fonts::install(&ctx);

        let mut laid = Vec::new();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            laid = layout_blocks(ui.ctx(), &raw.blocks, &HashMap::new(), content_width());
        });
        output.textures_delta.clear();
        assert!(!laid.is_empty());

        let metrics: Vec<_> = laid.iter().map(|b| b.metrics()).collect();
        let pages = paginate::paginate(&metrics, content_height(), raw.cover.is_some());
        assert!(pages.len() > 50);

        for page in &pages {
            if page.kind == paginate::PageKind::Cover {
                continue;
            }
            for item in &page.items {
                assert!(item.y < content_height() + 1.0);
                assert!(item.row_start < item.row_end);
            }
        }
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
        crate::render::fonts::install(&ctx);

        let mut cache = DisplayCache::default();
        let mut checked = 0usize;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let laid = layout_blocks(ui.ctx(), &raw.blocks, &HashMap::new(), content_width());
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
}
