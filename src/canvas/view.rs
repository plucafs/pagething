use eframe::egui::{
    self, Color32, Context, Event, Key, Modifiers, MouseWheelUnit, PointerButton, Rect, Response,
    Vec2,
};

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 8.0;
const WHEEL_ZOOM_STEP: f32 = 0.15;

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

    pub fn handle_shortcuts(&mut self, ctx: &Context) {
        if ctx.input_mut(|i| i.consume_key(Modifiers::CTRL, Key::Num0)) {
            self.reset();
        }
    }

    pub fn handle_input(&mut self, ctx: &Context, canvas_rect: Rect, response: &Response) {
        if response.dragged_by(PointerButton::Middle) {
            self.pan += response.drag_delta();
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
            let world_at_cursor = self.to_world(cursor);
            self.zoom =
                (self.zoom * (1.0 + line_scroll.y * WHEEL_ZOOM_STEP)).clamp(MIN_ZOOM, MAX_ZOOM);
            self.pan = cursor.to_vec2() - world_at_cursor * self.zoom;
        }
    }
}

pub const PAPER_BG: Color32 = Color32::from_gray(250);
