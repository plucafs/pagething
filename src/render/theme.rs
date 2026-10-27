use eframe::egui::{Color32, Theme};

use crate::persist::PageMode;

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

    pub const fn for_mode(mode: PageMode) -> Self {
        match mode {
            PageMode::Paper => Self::PAPER,
            PageMode::DarkPages => Self::DARK_PAGES,
        }
    }
}

pub const CANVAS_BG_DARK: Color32 = Color32::from_gray(38);
pub const CANVAS_BG_LIGHT: Color32 = Color32::from_gray(200);

pub fn canvas_bg(theme: Theme) -> Color32 {
    match theme {
        Theme::Dark => CANVAS_BG_DARK,
        Theme::Light => CANVAS_BG_LIGHT,
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
    fn for_mode_maps_settings() {
        assert_eq!(PageColors::for_mode(PageMode::Paper), PageColors::PAPER);
        assert_eq!(
            PageColors::for_mode(PageMode::DarkPages),
            PageColors::DARK_PAGES
        );
    }

    #[test]
    fn canvas_bg_follows_theme() {
        assert_eq!(canvas_bg(Theme::Dark), CANVAS_BG_DARK);
        assert_eq!(canvas_bg(Theme::Light), CANVAS_BG_LIGHT);
    }
}
