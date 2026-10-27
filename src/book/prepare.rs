use std::collections::HashMap;
use std::path::Path;

use eframe::egui;

use crate::book::blocks::{Block, parse_md};
use crate::book::convert::chapter_to_md;
use crate::book::load::{BookError, load as load_epub};
use crate::render::layout::decode_image;

pub struct PreparedBook {
    pub title: Option<String>,
    pub blocks: Vec<Block>,
    pub images: HashMap<String, egui::ColorImage>,
    pub cover: Option<egui::ColorImage>,
}

pub fn prepare(path: &Path) -> Result<PreparedBook, String> {
    let book = load_epub(path).map_err(|e: BookError| e.to_string())?;

    let mut blocks = Vec::new();
    for chapter in &book.chapters {
        let md = chapter_to_md(chapter).map_err(|e| e.to_string())?;
        blocks.extend(parse_md(&md));
    }

    let mut images = HashMap::new();
    for (href, bytes) in &book.images {
        if let Some(image) = decode_image(bytes) {
            images.insert(href.clone(), image);
        }
    }

    let cover = book.cover.as_deref().and_then(decode_image);

    Ok(PreparedBook {
        title: book.title,
        blocks,
        images,
        cover,
    })
}
