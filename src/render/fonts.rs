use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId};

const GELASIO: &str = "gelasio";
const GELASIO_BOLD: &str = "gelasio-bold";
const DEJAVU: &str = "dejavu";
const DEJAVU_BOLD: &str = "dejavu-bold";
pub const BOLD_FAMILY: &str = "pagething-bold";

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        GELASIO.to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/Gelasio-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        GELASIO_BOLD.to_owned(),
        Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/Gelasio-Bold.ttf"
        ))),
    );

    let existing: Vec<String> = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    let mut proportional = vec![GELASIO.to_owned()];
    proportional.extend(existing.into_iter().filter(|f| f != GELASIO));
    fonts
        .families
        .insert(FontFamily::Proportional, proportional.clone());

    let mut bold = vec![GELASIO_BOLD.to_owned()];
    bold.extend(proportional);
    fonts
        .families
        .insert(FontFamily::Name(BOLD_FAMILY.into()), bold);

    ctx.set_fonts(fonts);
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD_FAMILY.into()))
}
