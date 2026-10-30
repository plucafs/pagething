use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;

use eframe::egui;

use crate::book::blocks::{Block, parse_md};
use crate::book::convert::chapter_to_md;
use crate::book::load::BookError;
use crate::book::progress::{LoadProgress, LoadStage};
use crate::render::layout::decode_image;

pub struct PreparedImage {
    pub bytes: Vec<u8>,
    pub size: egui::Vec2,
}

pub struct PreparedBook {
    pub title: Option<String>,
    pub blocks: Vec<Block>,
    pub chapter_starts: Vec<usize>,
    pub images: HashMap<String, PreparedImage>,
    pub cover: Option<egui::ColorImage>,
}

pub fn prepare(path: &Path) -> Result<PreparedBook, String> {
    prepare_with_progress(path, &mut |_| {})
}

pub fn prepare_with_progress(
    path: &Path,
    progress: &mut impl FnMut(LoadProgress),
) -> Result<PreparedBook, String> {
    let book = crate::book::load::load_with_progress(path, progress)
        .map_err(|e: BookError| e.to_string())?;

    let mut blocks = Vec::new();
    let mut chapter_starts = Vec::new();
    progress(LoadProgress {
        stage: LoadStage::ConvertingChapters,
        current: 0,
        total: book.chapters.len(),
    });
    for (index, chapter) in book.chapters.iter().enumerate() {
        let md = chapter_to_md(chapter).map_err(|e| e.to_string())?;
        let chapter_blocks = parse_md(&md);
        if !chapter_blocks.is_empty() {
            chapter_starts.push(blocks.len());
            blocks.extend(chapter_blocks);
        }
        progress(LoadProgress {
            stage: LoadStage::ConvertingChapters,
            current: index + 1,
            total: book.chapters.len(),
        });
    }

    let mut images = HashMap::new();
    for (href, bytes) in book.images {
        if let Some(size) = image_dimensions(&bytes) {
            images.insert(href, PreparedImage { bytes, size });
        }
    }

    let cover = book.cover.as_deref().and_then(decode_image);

    Ok(PreparedBook {
        title: book.title,
        blocks,
        chapter_starts,
        images,
        cover,
    })
}

fn image_dimensions(bytes: &[u8]) -> Option<egui::Vec2> {
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let (width, height) = reader.into_dimensions().ok()?;
    (width > 0 && height > 0).then_some(egui::vec2(width as f32, height as f32))
}
