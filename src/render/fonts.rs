use std::sync::Arc;

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId};

const GELASIO: &str = "gelasio";
const GELASIO_BOLD: &str = "gelasio-bold";
pub const BOOK_FAMILY: &str = "pagething-book";
pub const BOOK_BOLD_FAMILY: &str = "pagething-book-bold";

pub fn definitions(rendering: Option<&fastframe_text::TextRendering>) -> FontDefinitions {
    let mut fonts = fastframe_fonts::FontSetup::default().definitions();
    let fallback: Vec<String> = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

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

    let mut book = vec![GELASIO.to_owned()];
    book.extend(fallback.iter().cloned());
    fonts
        .families
        .insert(FontFamily::Name(BOOK_FAMILY.into()), book.clone());

    let mut book_bold = vec![GELASIO_BOLD.to_owned()];
    book_bold.extend(book);
    fonts
        .families
        .insert(FontFamily::Name(BOOK_BOLD_FAMILY.into()), book_bold);

    if let Some(rendering) = rendering {
        rendering.apply_to(&mut fonts);
    }

    fonts
}

pub fn install(ctx: &egui::Context, rendering: &fastframe_text::TextRendering) {
    ctx.set_fonts(definitions(Some(rendering)));
}

#[cfg(test)]
pub fn install_for_tests(ctx: &egui::Context) {
    ctx.set_fonts(definitions(None));
}

pub fn book(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOOK_FAMILY.into()))
}

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOOK_BOLD_FAMILY.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gelasio_scoped_to_book_families() {
        let defs = definitions(None);
        let proportional = defs
            .families
            .get(&FontFamily::Proportional)
            .expect("proportional family");
        assert!(
            !proportional.iter().any(|f| f == GELASIO),
            "UI proportional family must not use Gelasio"
        );
        let book = defs
            .families
            .get(&FontFamily::Name(BOOK_FAMILY.into()))
            .expect("book family");
        assert_eq!(book[0], GELASIO);
        assert!(book.len() > 1, "book family keeps default fallback fonts");
        let book_bold = defs
            .families
            .get(&FontFamily::Name(BOOK_BOLD_FAMILY.into()))
            .expect("book bold family");
        assert_eq!(book_bold[0], GELASIO_BOLD);
    }

    #[test]
    fn font_ids_use_book_families() {
        assert_eq!(book(14.0).family, FontFamily::Name(BOOK_FAMILY.into()));
        assert_eq!(bold(14.0).family, FontFamily::Name(BOOK_BOLD_FAMILY.into()));
    }
}
