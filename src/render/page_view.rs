use std::sync::Arc;

use eframe::egui::{
    self, Align2, Color32, FontId, Mesh, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2,
};

use crate::book::paginate::{Page, PageKind};
use crate::canvas::grid::PAGE_SIZE;
use crate::canvas::view::{PAPER_BG, View};
use crate::render::layout::{DisplayCache, LaidBlock, LaidKind, MARGINS, RULE_HEIGHT};

const PAGE_BORDER: Color32 = Color32::from_gray(120);
const PAGE_NUMBER_COLOR: Color32 = Color32::from_gray(60);
const RULE_COLOR: Color32 = Color32::from_gray(150);

pub struct PageView<'a> {
    pub page: &'a Page,
    pub laid: &'a [LaidBlock],
    pub cover: Option<&'a Arc<egui::TextureHandle>>,
    pub number: usize,
    pub font_uv: Vec2,
}

pub fn paint_page(
    painter: &Painter,
    ctx: &egui::Context,
    cache: &mut DisplayCache,
    view: &View,
    screen_rect: Rect,
    pv: &PageView<'_>,
) {
    painter.rect_filled(screen_rect, 2.0, PAPER_BG);
    painter.rect_stroke(
        screen_rect,
        2.0,
        Stroke::new(1.0, PAGE_BORDER),
        StrokeKind::Outside,
    );

    let zoom = view.zoom;

    if pv.page.kind == PageKind::Cover {
        if let Some(cover) = pv.cover {
            paint_image_cover(painter, screen_rect, cover);
        }
        paint_page_number(painter, screen_rect, zoom, pv.number);
        return;
    }

    let content_origin = screen_rect.min + Vec2::new(MARGINS.left, MARGINS.top) * zoom;

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
                paint_rows(
                    painter,
                    &galley,
                    item.row_start,
                    item.row_end,
                    origin + Vec2::new(indent * zoom, 0.0),
                    paint_scale,
                    pv.font_uv,
                );
            }
            LaidKind::Image { texture, size } => {
                let rect = Rect::from_min_size(origin + Vec2::new(0.0, 0.0), *size * zoom);
                painter.image(
                    texture.id(),
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
            LaidKind::Rule => {
                let width = (PAGE_SIZE.x - MARGINS.left - MARGINS.right) * zoom;
                let rect = Rect::from_min_size(origin, Vec2::new(width, RULE_HEIGHT * zoom));
                painter.rect_filled(rect, 0.0, RULE_COLOR);
            }
        }
    }

    paint_page_number(painter, screen_rect, zoom, pv.number);
}

/// Reposition a row's pre-tessellated mesh for painting at `row_origin`.
/// `scale` is the factor mesh coordinates still need (1.0 for re-laid galleys).
///
/// `RowVisuals::mesh` stores UVs in font-texture *texels*; painting it as a raw
/// `Shape::Mesh` requires normalizing them to 0..1 first (the `Shape::Text` path
/// does this in `tessellate_text`, a raw mesh never gets that treatment).
fn transform_row_mesh(mesh: &Mesh, row_origin: Pos2, scale: f32, font_uv: Vec2) -> Mesh {
    let mut out = mesh.clone();
    for v in &mut out.vertices {
        v.pos = row_origin + v.pos.to_vec2() * scale;
        v.uv = (v.uv.to_vec2() * font_uv).to_pos2();
    }
    out
}

fn paint_rows(
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

fn paint_page_number(painter: &Painter, screen_rect: Rect, zoom: f32, number: usize) {
    let pos = egui::pos2(
        screen_rect.center().x,
        screen_rect.bottom() - MARGINS.bottom * 0.45 * zoom,
    );
    painter.text(
        pos,
        Align2::CENTER_BOTTOM,
        number.to_string(),
        FontId::proportional(11.0 * zoom),
        PAGE_NUMBER_COLOR,
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
}
