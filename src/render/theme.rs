use eframe::egui::{Color32, Theme};

use crate::persist::{AppBgMode, PageMode, Settings};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageColors {
    pub bg: Color32,
    pub border: Color32,
    pub body: Color32,
    pub heading: Color32,
    pub code_bg: Color32,
    pub page_number: Color32,
    pub rule: Color32,
}

impl PageColors {
    pub const PAPER: Self = Self {
        bg: Color32::from_rgb(247, 243, 234),
        border: Color32::from_rgb(150, 140, 125),
        body: Color32::from_rgb(43, 38, 33),
        heading: Color32::from_rgb(28, 24, 20),
        code_bg: Color32::from_rgb(236, 230, 218),
        page_number: Color32::from_rgb(110, 100, 90),
        rule: Color32::from_rgb(170, 160, 145),
    };

    pub const DARK_PAGES: Self = Self {
        bg: Color32::from_rgb(32, 32, 36),
        border: Color32::from_gray(70),
        body: Color32::from_gray(210),
        heading: Color32::from_gray(235),
        code_bg: Color32::from_gray(50),
        page_number: Color32::from_gray(150),
        rule: Color32::from_gray(90),
    };

    pub const SEPIA: Self = Self {
        bg: Color32::from_rgb(245, 236, 215),
        border: Color32::from_rgb(176, 154, 123),
        body: Color32::from_rgb(74, 56, 40),
        heading: Color32::from_rgb(56, 40, 28),
        code_bg: Color32::from_rgb(233, 220, 196),
        page_number: Color32::from_rgb(130, 110, 88),
        rule: Color32::from_rgb(186, 166, 136),
    };

    pub const fn for_mode(mode: PageMode) -> Self {
        match mode {
            PageMode::Paper => Self::PAPER,
            PageMode::DarkPages => Self::DARK_PAGES,
            PageMode::Sepia => Self::SEPIA,
            PageMode::Custom => Self::PAPER,
        }
    }

    pub fn custom(bg: Color32, font: Color32) -> Self {
        Self {
            bg,
            border: mix(bg, font, 0.45),
            body: font,
            heading: font,
            code_bg: mix(bg, font, 0.12),
            page_number: mix(bg, font, 0.55),
            rule: mix(bg, font, 0.40),
        }
    }
}

fn mix(a: Color32, b: Color32, amount: f32) -> Color32 {
    let channel =
        |a: u8, b: u8| (f32::from(a) * (1.0 - amount) + f32::from(b) * amount).round() as u8;
    Color32::from_rgb(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
    )
}

fn rgb(color: [u8; 3]) -> Color32 {
    Color32::from_rgb(color[0], color[1], color[2])
}

pub fn page_colors(settings: &Settings) -> PageColors {
    if settings.page_mode == PageMode::Custom {
        PageColors::custom(rgb(settings.page_bg), rgb(settings.page_font))
    } else {
        PageColors::for_mode(settings.page_mode)
    }
}

pub const CANVAS_BG_DARK: Color32 = Color32::from_gray(38);
pub const CANVAS_BG_LIGHT: Color32 = Color32::from_gray(200);

pub fn app_bg(settings: &Settings) -> Color32 {
    match settings.app_bg_mode {
        AppBgMode::Dark => CANVAS_BG_DARK,
        AppBgMode::Light => CANVAS_BG_LIGHT,
        AppBgMode::Custom => rgb(settings.app_bg),
    }
}

pub fn hint_color(theme: Theme) -> Color32 {
    match theme {
        Theme::Dark => Color32::from_gray(160),
        Theme::Light => Color32::from_gray(90),
    }
}

pub fn loading_color(theme: Theme) -> Color32 {
    match theme {
        Theme::Dark => Color32::WHITE,
        Theme::Light => Color32::from_gray(90),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gray(c: Color32) -> u8 {
        c.r()
    }

    #[test]
    fn paper_palette_is_dark_text_on_light_page() {
        let c = PageColors::PAPER;
        assert!(gray(c.bg) > gray(c.body));
        assert!(gray(c.bg) > gray(c.heading));
        assert!(gray(c.bg) < 255, "paper should not be pure white");
        assert!(gray(c.body) < gray(c.heading) + 30, "warm near-black ink");
    }

    #[test]
    fn dark_pages_palette_is_light_text_on_dark_page() {
        let c = PageColors::DARK_PAGES;
        assert!(gray(c.body) > gray(c.bg));
        assert!(gray(c.heading) > gray(c.bg));
    }

    #[test]
    fn sepia_palette_is_dark_text_on_warm_page() {
        let c = PageColors::SEPIA;
        assert!(gray(c.bg) > gray(c.body));
        assert!(gray(c.bg) > gray(c.heading));
        assert!(gray(c.bg) < 255, "sepia should not be pure white");
        // warmer (more red than blue) than paper
        assert!(c.bg.r() > c.bg.b(), "sepia bg should be warm");
    }

    #[test]
    fn for_mode_maps_settings() {
        assert_eq!(PageColors::for_mode(PageMode::Paper), PageColors::PAPER);
        assert_eq!(
            PageColors::for_mode(PageMode::DarkPages),
            PageColors::DARK_PAGES
        );
        assert_eq!(PageColors::for_mode(PageMode::Sepia), PageColors::SEPIA);
    }

    #[test]
    fn custom_page_colors_use_selected_ink_and_paper() {
        let bg = Color32::from_rgb(240, 230, 220);
        let font = Color32::from_rgb(30, 25, 20);
        let colors = PageColors::custom(bg, font);

        assert_eq!(colors.bg, bg);
        assert_eq!(colors.body, font);
        assert_eq!(colors.heading, font);
        assert!(colors.code_bg.r() < bg.r());
        assert!(colors.border.r() < bg.r());
        assert!(colors.page_number.r() < bg.r());
        assert!(colors.rule.r() < bg.r());
    }

    #[test]
    fn settings_resolve_custom_page_and_app_colors() {
        let settings = Settings {
            page_mode: PageMode::Custom,
            page_bg: [230, 220, 210],
            page_font: [20, 18, 16],
            app_bg_mode: AppBgMode::Custom,
            app_bg: [12, 34, 56],
            ..Settings::default()
        };

        assert_eq!(page_colors(&settings).bg, Color32::from_rgb(230, 220, 210));
        assert_eq!(page_colors(&settings).body, Color32::from_rgb(20, 18, 16));
        assert_eq!(app_bg(&settings), Color32::from_rgb(12, 34, 56));
        assert_eq!(app_bg(&Settings::default()), CANVAS_BG_DARK);
    }
}
