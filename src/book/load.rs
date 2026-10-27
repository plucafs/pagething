use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum BookError {
    #[error("failed to open epub: {0}")]
    Open(String),
    #[error("failed to read resource: {0}")]
    Resource(String),
}

pub struct Chapter {
    pub href: String,
    pub xhtml: String,
}

pub struct Book {
    pub title: Option<String>,
    pub chapters: Vec<Chapter>,
    pub images: HashMap<String, Vec<u8>>,
    pub cover: Option<Vec<u8>>,
}

pub fn load(path: &Path) -> Result<Book, BookError> {
    let epub = rbook::Epub::open(path).map_err(|e| BookError::Open(e.to_string()))?;

    let title = epub.metadata().title().map(|t| t.value().to_string());

    let mut chapters = Vec::new();
    let mut reader = epub.reader();
    while let Some(Ok(data)) = reader.read_next() {
        let href = normalize_path(&data.manifest_entry().href().decode());
        chapters.push(Chapter {
            href,
            xhtml: data.content().to_string(),
        });
    }

    let mut images = HashMap::new();
    for image in epub.manifest().images() {
        let key = normalize_path(&image.href().decode());
        let bytes = image
            .read_bytes()
            .map_err(|e| BookError::Resource(e.to_string()))?;
        images.insert(key, bytes);
    }

    let cover = epub
        .manifest()
        .cover_image()
        .map(|entry| entry.read_bytes())
        .transpose()
        .map_err(|e| BookError::Resource(e.to_string()))?;

    Ok(Book {
        title,
        chapters,
        images,
        cover,
    })
}

/// Percent-decode and normalize an EPUB resource path to `/seg/seg` form.
pub fn normalize_path(raw: &str) -> String {
    let decoded = raw.split(['?', '#']).next().unwrap_or(raw);
    let mut out: Vec<&str> = Vec::new();
    for seg in decoded.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    format!("/{}", out.join("/"))
}

/// Resolve an href found inside a chapter file to a normalized resource path.
pub fn resolve_href(chapter_href: &str, src: &str) -> String {
    let raw = src.split(['?', '#']).next().unwrap_or(src);
    let decoded = percent_decode(raw);
    if decoded.starts_with('/') {
        return normalize_path(&decoded);
    }
    let base = chapter_href
        .rsplit_once('/')
        .map(|(dir, _)| format!("{dir}/{decoded}"))
        .unwrap_or_else(|| decoded.to_string());
    normalize_path(&base)
}

fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = raw.get(i + 1..i + 3)
            && let Ok(byte) = u8::from_str_radix(hex, 16)
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_paths() {
        assert_eq!(
            normalize_path("/EPUB/text/ch1.xhtml"),
            "/EPUB/text/ch1.xhtml"
        );
        assert_eq!(
            normalize_path("EPUB//text/./ch1.xhtml"),
            "/EPUB/text/ch1.xhtml"
        );
        assert_eq!(normalize_path("/EPUB/text/../img/a.png"), "/EPUB/img/a.png");
    }

    #[test]
    fn resolves_relative_hrefs() {
        assert_eq!(
            resolve_href("/EPUB/text/ch1.xhtml", "../images/fig1.png"),
            "/EPUB/images/fig1.png"
        );
        assert_eq!(
            resolve_href("/EPUB/text/ch1.xhtml", "fig1%20a.png"),
            "/EPUB/text/fig1 a.png"
        );
        assert_eq!(
            resolve_href("/EPUB/text/ch1.xhtml", "/img/a.png"),
            "/img/a.png"
        );
    }

    #[test]
    fn loads_real_epub() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/books/pg1342.epub");
        let book = load(&path).expect("load pg1342");
        assert_eq!(book.title.as_deref(), Some("Pride and Prejudice"));
        assert!(book.chapters.len() > 5);
        assert!(book.chapters.iter().any(|c| c.xhtml.contains("<body")));
    }
}
