use eframe::egui::{
    self, Context, Event, Key, KeyboardShortcut, Modifiers, MouseWheelUnit, PointerButton, Pos2,
    Rect, Response, Vec2,
};

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 8.0;
const WHEEL_ZOOM_STEP: f32 = 0.15;
const KEY_ZOOM_STEP: f32 = 1.2;

#[derive(Debug, Clone)]
pub struct View {
    pub pan: Vec2,
    pub zoom: f32,
}

impl Default for View {
    fn default() -> Self {
        Self {
            pan: Vec2::new(48.0, 48.0),
            zoom: 1.0,
        }
    }
}

impl View {
    pub fn to_screen(&self, world: Vec2) -> egui::Pos2 {
        (world * self.zoom + self.pan).to_pos2()
    }

    pub fn to_world(&self, screen: egui::Pos2) -> Vec2 {
        (screen.to_vec2() - self.pan) / self.zoom
    }

    pub fn to_screen_rect(&self, world: Rect) -> Rect {
        Rect::from_min_max(
            self.to_screen(world.min.to_vec2()),
            self.to_screen(world.max.to_vec2()),
        )
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn zoom_at(&mut self, anchor: Pos2, factor: f32) {
        let world_at_anchor = self.to_world(anchor);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = anchor.to_vec2() - world_at_anchor * self.zoom;
    }

    pub fn reset_zoom_at(&mut self, anchor: Pos2) {
        self.zoom_at(anchor, 1.0 / self.zoom.max(MIN_ZOOM));
    }

    pub fn handle_shortcuts(&mut self, ctx: &Context, canvas_rect: Rect) -> bool {
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::Num0)) {
            self.reset();
            return true;
        }

        let zoom_in = [
            KeyboardShortcut::new(Modifiers::CTRL, Key::Plus),
            KeyboardShortcut::new(Modifiers::CTRL, Key::Equals),
            KeyboardShortcut::new(Modifiers::CTRL | Modifiers::SHIFT, Key::Plus),
        ]
        .into_iter()
        .any(|shortcut| ctx.input_mut(|i| i.consume_shortcut(&shortcut)));
        if zoom_in {
            self.zoom_at(canvas_rect.center(), KEY_ZOOM_STEP);
            return true;
        }
        if ctx
            .input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::CTRL, Key::Minus)))
        {
            self.zoom_at(canvas_rect.center(), 1.0 / KEY_ZOOM_STEP);
            return true;
        }
        false
    }

    pub fn handle_input(&mut self, ctx: &Context, canvas_rect: Rect, response: &Response) {
        if response.dragged_by(PointerButton::Middle) {
            self.pan += response.drag_delta();
        }

        // Scroll/zoom only when the canvas is the top widget under the pointer:
        // a modal (or menu) covering the pointer must keep its own scrolling.
        if !response.hovered() {
            return;
        }

        let pointer_pos = ctx
            .pointer_latest_pos()
            .filter(|p| canvas_rect.contains(*p));

        let (line_scroll, pixel_scroll) = ctx.input(|i| {
            let mut line = Vec2::ZERO;
            let mut pixel = Vec2::ZERO;
            for event in &i.raw.events {
                if let Event::MouseWheel { unit, delta, .. } = event {
                    match unit {
                        MouseWheelUnit::Line | MouseWheelUnit::Page => line += *delta,
                        MouseWheelUnit::Point => pixel += *delta,
                    }
                }
            }
            (line, pixel)
        });

        if pixel_scroll != Vec2::ZERO {
            self.pan += pixel_scroll;
        }

        if line_scroll.y != 0.0
            && let Some(cursor) = pointer_pos
        {
            self.zoom_at(cursor, 1.0 + line_scroll.y * WHEEL_ZOOM_STEP);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_the_world_point_under_anchor_fixed() {
        let mut view = View {
            pan: Vec2::new(73.0, -22.0),
            zoom: 1.4,
        };
        let anchor = Pos2::new(310.0, 180.0);
        let world_before = view.to_world(anchor);

        view.zoom_at(anchor, 1.2);

        assert!((view.zoom - 1.68).abs() < 1e-5);
        assert!((view.to_world(anchor) - world_before).length() < 1e-4);
    }

    #[test]
    fn reset_zoom_keeps_pan_anchor_and_sets_zoom_to_one() {
        let mut view = View {
            pan: Vec2::new(-80.0, 125.0),
            zoom: 2.4,
        };
        let anchor = Pos2::new(200.0, 260.0);
        let world_before = view.to_world(anchor);

        view.reset_zoom_at(anchor);

        assert_eq!(view.zoom, 1.0);
        assert!((view.to_world(anchor) - world_before).length() < 1e-4);
    }
}
