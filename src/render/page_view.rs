use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui::{self, Align2, Color32, Mesh, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};

use crate::book::paginate::{Page, PageKind};
use crate::canvas::view::View;
use crate::features::selection;
use crate::persist::{PageMode, PageTexture};
use crate::render::layout::{DisplayCache, LaidBlock, LaidKind, Margins, RULE_HEIGHT};
use crate::render::theme::PageColors;

pub const SEARCH_HL: Color32 = Color32::from_rgba_premultiplied(140, 112, 33, 140);
pub const SEARCH_CURRENT_HL: Color32 = Color32::from_rgba_premultiplied(190, 70, 15, 180);
pub const SELECTION_HL: Color32 = Color32::from_rgba_premultiplied(26, 60, 110, 110);
const TEXTURE_TILE_WORLD_SIZE: f32 = 128.0;
const TEXTURE_TILE_PIXELS: usize = 128;

pub struct PageTextures {
    fine_grain: egui::TextureHandle,
    paper_fibers: egui::TextureHandle,
    linen_weave: egui::TextureHandle,
}

impl PageTextures {
    pub fn load(ctx: &egui::Context) -> Self {
        let load = |name, texture| {
            ctx.load_texture(
                name,
                texture_image(texture),
                egui::TextureOptions::LINEAR_REPEAT,
            )
        };
        Self {
            fine_grain: load("page-texture-fine-grain", PageTexture::FineGrain),
            paper_fibers: load("page-texture-paper-fibers", PageTexture::PaperFibers),
            linen_weave: load("page-texture-linen-weave", PageTexture::LinenWeave),
        }
    }

    pub fn get(&self, texture: PageTexture) -> Option<&egui::TextureHandle> {
        match texture {
            PageTexture::Plain => None,
            PageTexture::FineGrain => Some(&self.fine_grain),
            PageTexture::PaperFibers => Some(&self.paper_fibers),
            PageTexture::LinenWeave => Some(&self.linen_weave),
        }
    }
}

/// User highlight wash: visible on light pages, muted on dark pages.
pub fn user_hl(mode: PageMode, bg: Color32) -> Color32 {
    match mode {
        PageMode::DarkPages => Color32::from_rgba_premultiplied(130, 118, 70, 110),
        PageMode::Paper | PageMode::Sepia => Color32::from_rgba_premultiplied(255, 230, 90, 110),
        PageMode::Custom
            if (u16::from(bg.r()) * 299 + u16::from(bg.g()) * 587 + u16::from(bg.b()) * 114)
                / 1000
                > 128 =>
        {
            Color32::from_rgba_premultiplied(255, 230, 90, 110)
        }
        PageMode::Custom => Color32::from_rgba_premultiplied(130, 118, 70, 110),
    }
}

pub struct PageView<'a> {
    pub page: &'a Page,
    pub laid: &'a [LaidBlock],
    pub cover: Option<&'a Arc<egui::TextureHandle>>,
    pub textures: &'a HashMap<String, Arc<egui::TextureHandle>>,
    pub number: usize,
    pub font_uv: Vec2,
    pub margins: Margins,
    pub page_size: Vec2,
    pub colors: PageColors,
    pub page_texture: Option<&'a egui::TextureHandle>,
    /// `(item index, char a, char b, color)` painted under the text.
    pub highlights: &'a [(usize, usize, usize, Color32)],
}

pub fn paint_page(
    painter: &Painter,
    ctx: &egui::Context,
    cache: &mut DisplayCache,
    view: &View,
    screen_rect: Rect,
    pv: &PageView<'_>,
) {
    painter.rect_filled(screen_rect, 2.0, pv.colors.bg);
    if let Some(texture) = pv.page_texture {
        let uv = Rect::from_min_max(
            Pos2::ZERO,
            egui::pos2(
                pv.page_size.x / TEXTURE_TILE_WORLD_SIZE,
                pv.page_size.y / TEXTURE_TILE_WORLD_SIZE,
            ),
        );
        painter.image(texture.id(), screen_rect, uv, texture_tint(pv.colors.bg));
    }
    painter.rect_stroke(
        screen_rect,
        2.0,
        Stroke::new(1.0, pv.colors.border),
        StrokeKind::Outside,
    );

    let zoom = view.zoom;
    paint_highlights(painter, screen_rect, pv, zoom);

    if pv.page.kind == PageKind::Cover {
        if let Some(cover) = pv.cover {
            paint_image_cover(painter, screen_rect, cover);
        }
        paint_page_number(
            painter,
            screen_rect,
            zoom,
            pv.number,
            pv.margins.bottom,
            pv.colors.page_number,
        );
        return;
    }

    let content_origin = screen_rect.min + Vec2::new(pv.margins.left, pv.margins.top) * zoom;

    for item in &pv.page.items {
        let Some(block) = pv.laid.get(item.block) else {
            continue;
        };
        let origin = content_origin + Vec2::new(0.0, item.y * zoom);

        match &block.kind {
            LaidKind::Text {
                galley,
                job,
                indent,
                row_heights: _,
            } => {
                let (galley, paint_scale) = cache.galley_for(ctx, item.block, galley, job, zoom);
                let piece_origin = origin + Vec2::new(indent * zoom, 0.0);
                if paint_scale == 1.0 {
                    paint_text_piece(
                        painter,
                        &galley,
                        item.row_start,
                        item.row_end,
                        piece_origin,
                        pv.colors.body,
                    );
                } else {
                    paint_rows_mesh(
                        painter,
                        &galley,
                        item.row_start,
                        item.row_end,
                        piece_origin,
                        paint_scale,
                        pv.font_uv,
                    );
                }
            }
            LaidKind::Image { key, size } => {
                let rect = Rect::from_min_size(origin + Vec2::new(0.0, 0.0), *size * zoom);
                if let Some(texture) = pv.textures.get(key) {
                    painter.image(
                        texture.id(),
                        rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                } else {
                    painter.rect_filled(rect, 1.0, pv.colors.code_bg);
                    painter.rect_stroke(
                        rect,
                        1.0,
                        Stroke::new(1.0, pv.colors.rule),
                        StrokeKind::Inside,
                    );
                }
            }
            LaidKind::Rule => {
                let width = (pv.page_size.x - pv.margins.left - pv.margins.right) * zoom;
                let rect = Rect::from_min_size(origin, Vec2::new(width, RULE_HEIGHT * zoom));
                painter.rect_filled(rect, 0.0, pv.colors.rule);
            }
        }
    }

    paint_page_number(
        painter,
        screen_rect,
        zoom,
        pv.number,
        pv.margins.bottom,
        pv.colors.page_number,
    );
}

fn texture_tint(bg: Color32) -> Color32 {
    let luminance =
        (f32::from(bg.r()) * 0.299 + f32::from(bg.g()) * 0.587 + f32::from(bg.b()) * 0.114) as u8;
    if luminance > 128 {
        Color32::from_rgba_unmultiplied(52, 42, 31, 18)
    } else {
        Color32::from_rgba_unmultiplied(235, 229, 211, 18)
    }
}

fn texture_image(texture: PageTexture) -> egui::ColorImage {
    let mut mask = vec![0_u8; TEXTURE_TILE_PIXELS * TEXTURE_TILE_PIXELS];
    let mut seed = match texture {
        PageTexture::Plain => 0,
        PageTexture::FineGrain => 0x73a4_91c5,
        PageTexture::PaperFibers => 0xb18e_2d63,
        PageTexture::LinenWeave => 0x39d2_7f41,
    };

    match texture {
        PageTexture::Plain => {}
        PageTexture::FineGrain => {
            for alpha in &mut mask {
                if next_random(&mut seed) % 5 == 0 {
                    *alpha = 255;
                }
            }
        }
        PageTexture::PaperFibers => {
            for _ in 0..80 {
                let x = (next_random(&mut seed) as usize) % TEXTURE_TILE_PIXELS;
                let y = (next_random(&mut seed) as usize) % TEXTURE_TILE_PIXELS;
                let length = 2 + (next_random(&mut seed) as usize % 7);
                let dx = (next_random(&mut seed) % 5) as i32 - 2;
                draw_fiber(&mut mask, x as i32, y as i32, dx, length as i32);
            }
        }
        PageTexture::LinenWeave => {
            for y in 0..TEXTURE_TILE_PIXELS {
                for x in 0..TEXTURE_TILE_PIXELS {
                    let vertical = x % 4 == 0;
                    let horizontal = y % 4 == 0;
                    if vertical || horizontal {
                        let variation = (next_random(&mut seed) % 28) as u8;
                        mask[y * TEXTURE_TILE_PIXELS + x] = 65 + variation;
                    }
                }
            }
        }
    }

    let mut rgba = Vec::with_capacity(mask.len() * 4);
    for alpha in mask {
        rgba.extend_from_slice(&[255, 255, 255, alpha]);
    }
    egui::ColorImage::from_rgba_unmultiplied([TEXTURE_TILE_PIXELS, TEXTURE_TILE_PIXELS], &rgba)
}

fn next_random(seed: &mut u32) -> u32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    *seed
}

fn draw_fiber(mask: &mut [u8], x: i32, y: i32, dx: i32, dy: i32) {
    let (mut x0, mut y0) = (x, y);
    let (x1, y1) = (x + dx, y + dy);
    let step_x = if x0 < x1 { 1 } else { -1 };
    let step_y = if y0 < y1 { 1 } else { -1 };
    let delta_x = (x1 - x0).abs();
    let delta_y = -(y1 - y0).abs();
    let mut error = delta_x + delta_y;

    loop {
        if (0..TEXTURE_TILE_PIXELS as i32).contains(&x0)
            && (0..TEXTURE_TILE_PIXELS as i32).contains(&y0)
        {
            mask[y0 as usize * TEXTURE_TILE_PIXELS + x0 as usize] = 255;
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let twice_error = 2 * error;
        if twice_error >= delta_y {
            error += delta_y;
            x0 += step_x;
        }
        if twice_error <= delta_x {
            error += delta_x;
            y0 += step_y;
        }
    }
}

/// Highlight rects under the text: `(item, char a, char b, color)` spans.
fn paint_highlights(painter: &Painter, screen_rect: Rect, pv: &PageView<'_>, zoom: f32) {
    for &(item_index, a, b, color) in pv.highlights {
        let Some(item) = pv.page.items.get(item_index) else {
            continue;
        };
        let Some(block) = pv.laid.get(item.block) else {
            continue;
        };
        let LaidKind::Text { galley, indent, .. } = &block.kind else {
            continue;
        };
        let Some(row_first) = galley.rows.get(item.row_start) else {
            continue;
        };
        let base = row_first.pos;
        let origin =
            screen_rect.min + Vec2::new(pv.margins.left + *indent, pv.margins.top + item.y) * zoom;
        for rect in selection::span_rects(galley, item.row_start, item.row_end, a, b) {
            let min = origin + (rect.min.to_vec2() - base.to_vec2()) * zoom;
            let max = origin + (rect.max.to_vec2() - base.to_vec2()) * zoom;
            if max.x > min.x && max.y > min.y {
                painter.rect_filled(Rect::from_min_max(min, max), 0.0, color);
            }
        }
    }
}

/// Screen rect of a piece (a block's rows `[row_start..row_end]`) painted at
/// `origin`, plus the galley position that puts `row_start` there.
pub fn piece_rect(
    galley: &egui::text::Galley,
    row_start: usize,
    row_end: usize,
    origin: Pos2,
) -> Option<(Rect, Pos2)> {
    if row_start >= row_end || row_start >= galley.rows.len() {
        return None;
    }
    let row_end = row_end.min(galley.rows.len());
    let base = galley.rows[row_start].pos;
    let bottom = galley.rows[row_end - 1].rect().bottom();
    let height = (bottom - base.y).max(1.0);
    let width = galley.rect.width().max(1.0) + 8.0;
    let clip = Rect::from_min_size(origin, Vec2::new(width, height));
    let galley_pos = origin - base.to_vec2();
    Some((clip, galley_pos))
}

/// Paint a piece via `TextShape`: no mesh cloning, no per-vertex work on our
/// side — egui culls rows outside the clip rect and batches the rest into one
/// mesh. Requires a galley already laid out at screen scale (`paint_scale == 1`).
fn paint_text_piece(
    painter: &Painter,
    galley: &Arc<egui::text::Galley>,
    row_start: usize,
    row_end: usize,
    origin: Pos2,
    fallback: Color32,
) {
    let Some((clip, galley_pos)) = piece_rect(galley, row_start, row_end, origin) else {
        return;
    };
    painter
        .with_clip_rect(clip)
        .add(egui::Shape::Text(egui::epaint::TextShape::new(
            galley_pos,
            Arc::clone(galley),
            fallback,
        )));
}

/// Reposition a row's pre-tessellated mesh for painting at `row_origin`.
/// `scale` is the factor mesh coordinates still need (1.0 for re-laid galleys).
///
/// Fallback path only (galley row-count drift after re-layout): the fast path
/// is [`paint_text_piece`]. When used, `RowVisuals::mesh` UVs are in font-texture
/// *texels* and must be normalized to 0..1 (the `Shape::Text` path does this in
/// `tessellate_text`; a raw mesh never gets that treatment).
fn transform_row_mesh(mesh: &Mesh, row_origin: Pos2, scale: f32, font_uv: Vec2) -> Mesh {
    let mut out = mesh.clone();
    for v in &mut out.vertices {
        v.pos = row_origin + v.pos.to_vec2() * scale;
        v.uv = (v.uv.to_vec2() * font_uv).to_pos2();
    }
    out
}

fn paint_rows_mesh(
    painter: &Painter,
    galley: &egui::text::Galley,
    row_start: usize,
    row_end: usize,
    origin: Pos2,
    paint_scale: f32,
    font_uv: Vec2,
) {
    if row_start >= row_end || row_start >= galley.rows.len() {
        return;
    }
    let base = galley.rows[row_start].pos;
    for row in &galley.rows[row_start..row_end.min(galley.rows.len())] {
        if row.visuals.mesh.is_empty() {
            continue;
        }
        let row_origin = origin + (row.pos - base) * paint_scale;
        let mesh = transform_row_mesh(&row.visuals.mesh, row_origin, paint_scale, font_uv);
        painter.add(egui::Shape::Mesh(mesh.into()));
    }
}

fn paint_image_cover(painter: &Painter, rect: Rect, texture: &egui::TextureHandle) {
    let size = texture.size_vec2();
    if size.x <= 0.0 || size.y <= 0.0 {
        return;
    }
    let rect_aspect = rect.width() / rect.height();
    let img_aspect = size.x / size.y;
    let uv = if img_aspect > rect_aspect {
        let w = rect_aspect / img_aspect;
        Rect::from_min_max(
            egui::pos2((1.0 - w) / 2.0, 0.0),
            egui::pos2((1.0 + w) / 2.0, 1.0),
        )
    } else {
        let h = img_aspect / rect_aspect;
        Rect::from_min_max(
            egui::pos2(0.0, (1.0 - h) / 2.0),
            egui::pos2(1.0, (1.0 + h) / 2.0),
        )
    };
    painter.image(texture.id(), rect, uv, Color32::WHITE);
}

fn paint_page_number(
    painter: &Painter,
    screen_rect: Rect,
    zoom: f32,
    number: usize,
    bottom_margin: f32,
    color: Color32,
) {
    let pos = egui::pos2(
        screen_rect.center().x,
        screen_rect.bottom() - bottom_margin * 0.45 * zoom,
    );
    painter.text(
        pos,
        Align2::CENTER_BOTTOM,
        number.to_string(),
        crate::render::fonts::book(16.0 * zoom),
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Color32, pos2};

    fn texel_mesh() -> Mesh {
        let mut mesh = Mesh::default();
        mesh.vertices.push(egui::epaint::Vertex {
            pos: pos2(10.0, 5.0),
            uv: pos2(512.0, 256.0),
            color: Color32::BLACK,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: pos2(20.0, 5.0),
            uv: pos2(1023.0, 256.0),
            color: Color32::BLACK,
        });
        mesh.indices.extend_from_slice(&[0, 1, 0]);
        mesh
    }

    #[test]
    fn row_mesh_transform_scales_positions_and_normalizes_uvs() {
        let font_uv = Vec2::new(1.0 / 1024.0, 1.0 / 2048.0);
        let out = transform_row_mesh(&texel_mesh(), pos2(100.0, 200.0), 2.0, font_uv);

        assert_eq!(out.vertices[0].pos, pos2(120.0, 210.0));
        assert_eq!(out.vertices[1].pos, pos2(140.0, 210.0));

        assert!((out.vertices[0].uv - pos2(0.5, 0.125)).length() < 1e-4);
        assert!((out.vertices[1].uv - pos2(1023.0 / 1024.0, 0.125)).length() < 1e-4);
    }

    #[test]
    fn row_mesh_transform_produces_uv_in_unit_range() {
        let font_uv = Vec2::new(1.0 / 1024.0, 1.0 / 2048.0);
        let out = transform_row_mesh(&texel_mesh(), pos2(0.0, 0.0), 1.0, font_uv);
        for v in &out.vertices {
            assert!(
                v.uv.x >= 0.0 && v.uv.x <= 1.0,
                "uv.x out of range: {}",
                v.uv.x
            );
            assert!(
                v.uv.y >= 0.0 && v.uv.y <= 1.0,
                "uv.y out of range: {}",
                v.uv.y
            );
        }
    }

    #[test]
    fn page_texture_variants_generate_distinct_repeatable_masks() {
        let masks = [
            texture_image(PageTexture::Plain),
            texture_image(PageTexture::FineGrain),
            texture_image(PageTexture::PaperFibers),
            texture_image(PageTexture::LinenWeave),
        ];

        assert!(
            masks
                .iter()
                .all(|image| { image.size == [TEXTURE_TILE_PIXELS, TEXTURE_TILE_PIXELS] })
        );
        assert!(masks[0].pixels.iter().all(|pixel| pixel.a() == 0));
        for image in &masks[1..] {
            assert!(image.pixels.iter().any(|pixel| pixel.a() > 0));
        }
        assert_ne!(masks[1].pixels, masks[2].pixels);
        assert_ne!(masks[2].pixels, masks[3].pixels);
        assert_eq!(
            texture_image(PageTexture::FineGrain).pixels,
            masks[1].pixels
        );
    }

    #[test]
    fn piece_rect_matches_row_band() {
        let ctx = egui::Context::default();
        crate::render::fonts::install_for_tests(&ctx);

        let mut captured = None;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let mut job = egui::text::LayoutJob::default();
            job.wrap.max_width = 120.0;
            job.append(
                "word ".repeat(30).as_str(),
                0.0,
                egui::TextFormat::default(),
            );
            let galley = ui.ctx().fonts_mut(|f| f.layout_job(job));
            assert!(galley.rows.len() >= 3);

            let origin = pos2(50.0, 60.0);
            let (clip, galley_pos) = piece_rect(&galley, 1, 3, origin).unwrap();
            let expected_h = galley.rows[2].rect().bottom() - galley.rows[1].pos.y;
            let expected_pos = origin - galley.rows[1].pos.to_vec2();

            captured = Some((
                clip,
                galley_pos,
                expected_h,
                expected_pos,
                galley.rect.width(),
            ));
        });
        output.textures_delta.clear();

        let (clip, galley_pos, expected_h, expected_pos, galley_width) = captured.unwrap();
        assert_eq!(clip.min, pos2(50.0, 60.0));
        assert!((clip.height() - expected_h).abs() < 1e-4);
        assert!((clip.width() - (galley_width + 8.0)).abs() < 1e-4);
        assert!((galley_pos - expected_pos).length() < 1e-4);
    }

    #[test]
    fn piece_rect_rejects_empty_or_out_of_range() {
        let ctx = egui::Context::default();
        crate::render::fonts::install_for_tests(&ctx);

        let mut bad = (false, false, false);
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let mut job = egui::text::LayoutJob::default();
            job.wrap.max_width = 120.0;
            job.append(
                "word ".repeat(30).as_str(),
                0.0,
                egui::TextFormat::default(),
            );
            let galley = ui.ctx().fonts_mut(|f| f.layout_job(job));
            let n = galley.rows.len();
            let origin = pos2(0.0, 0.0);
            bad = (
                piece_rect(&galley, 1, 1, origin).is_none(),
                piece_rect(&galley, 2, 1, origin).is_none(),
                piece_rect(&galley, n, n + 5, origin).is_none(),
            );
        });
        output.textures_delta.clear();
        assert_eq!(bad, (true, true, true));
    }
}
