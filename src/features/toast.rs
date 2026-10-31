use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Color32, Id, LayerId, Order, Rect, Vec2};

const TOAST_DURATION: Duration = Duration::from_millis(2200);
const FADE_DURATION: Duration = Duration::from_millis(300);

#[derive(Default)]
pub struct Toasts {
    current: Option<(String, Instant)>,
}

impl Toasts {
    pub fn show(&mut self, message: impl Into<String>) {
        self.current = Some((message.into(), Instant::now()));
    }

    pub fn paint(&mut self, ctx: &egui::Context) {
        let Some((message, started)) = &self.current else {
            return;
        };
        let elapsed = started.elapsed();
        if elapsed >= TOAST_DURATION {
            self.current = None;
            return;
        }

        let fade_start = TOAST_DURATION - FADE_DURATION;
        let alpha = if elapsed > fade_start {
            (TOAST_DURATION - elapsed).as_secs_f32() / FADE_DURATION.as_secs_f32()
        } else {
            1.0
        }
        .clamp(0.0, 1.0);

        let screen = ctx.content_rect();
        let width = (message.chars().count() as f32 * 8.0 + 32.0).clamp(120.0, 320.0);
        let rect = Rect::from_min_size(
            screen.right_bottom() - Vec2::new(width + 16.0, 54.0),
            Vec2::new(width, 38.0),
        );
        let style = ctx.style_of(ctx.theme());
        let visuals = &style.visuals;
        let fill = with_alpha(visuals.window_fill, alpha);
        let text = with_alpha(visuals.text_color(), alpha);
        let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("action-toast")));
        painter.rect_filled(rect, 7.0, fill);
        painter.rect_stroke(
            rect,
            7.0,
            egui::Stroke::new(1.0, with_alpha(visuals.window_stroke().color, alpha)),
            egui::StrokeKind::Inside,
        );
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            message,
            egui::FontId::proportional(14.0),
            text,
        );

        let until_fade = fade_start.saturating_sub(elapsed);
        let repaint_after = if until_fade.is_zero() {
            Duration::from_millis(16)
        } else {
            until_fade
        };
        ctx.request_repaint_after(repaint_after);
    }
}

fn with_alpha(color: Color32, alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        (color.a() as f32 * alpha).round() as u8,
    )
}
