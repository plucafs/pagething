use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId};

const DEJAVU: &str = "dejavu";
const DEJAVU_BOLD: &str = "dejavu-bold";
pub const BOLD_FAMILY: &str = "pagething-bold";

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        DEJAVU.to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/DejaVuSans.ttf"
        ))),
    );
    fonts.font_data.insert(
        DEJAVU_BOLD.to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/DejaVuSans-Bold.ttf"
        ))),
    );

    let existing: Vec<String> = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    let mut proportional = vec![DEJAVU.to_owned()];
    proportional.extend(existing.into_iter().filter(|f| f != DEJAVU));
    fonts
        .families
        .insert(FontFamily::Proportional, proportional.clone());

    let mut bold = vec![DEJAVU_BOLD.to_owned()];
    bold.extend(proportional);
    fonts
        .families
        .insert(FontFamily::Name(BOLD_FAMILY.into()), bold);

    ctx.set_fonts(fonts);
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD_FAMILY.into()))
}
