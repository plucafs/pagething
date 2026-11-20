use crate::persist::{Bookmark, Highlight, Postit};

pub fn markdown(
    title: &str,
    notes: &[Postit],
    highlights: &[Highlight],
    bookmarks: &[Bookmark],
) -> String {
    let mut out = format!("# Annotations: {title}\n\n");

    out.push_str("## Bookmarks\n\n");
    if bookmarks.is_empty() {
        out.push_str("No bookmarks.\n\n");
    }
    for bookmark in bookmarks {
        out.push_str(&format!("- Page {}\n", bookmark.page));
    }
    out.push('\n');

    out.push_str("## Highlights\n\n");
    if highlights.is_empty() {
        out.push_str("No highlights.\n\n");
    }
    for highlight in highlights {
        let quote = highlight
            .anchor
            .as_ref()
            .map(|anchor| anchor.quote.as_str())
            .unwrap_or("");
        out.push_str(&format!("- Page {}: \"{}\"\n", highlight.page, quote));
    }
    out.push('\n');

    out.push_str("## Notes\n\n");
    if notes.is_empty() {
        out.push_str("No notes.\n");
    }
    for note in notes {
        out.push_str(&format!("### Page {}\n\n{}\n\n", note.page, note.text));
        for reply in &note.replies {
            out.push_str(&format!("- Reply: {}\n", reply.text));
        }
        if !note.replies.is_empty() {
            out.push('\n');
        }
    }
    out
}

pub fn html(
    title: &str,
    notes: &[Postit],
    highlights: &[Highlight],
    bookmarks: &[Bookmark],
) -> String {
    let mut out = format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>Annotations: {}</title><body><h1>Annotations: {}</h1>",
        escape_html(title),
        escape_html(title)
    );
    out.push_str("<h2>Bookmarks</h2><ul>");
    for bookmark in bookmarks {
        out.push_str(&format!("<li>Page {}</li>", bookmark.page));
    }
    out.push_str("</ul><h2>Highlights</h2><ul>");
    for highlight in highlights {
        let quote = highlight
            .anchor
            .as_ref()
            .map(|anchor| anchor.quote.as_str())
            .unwrap_or("");
        out.push_str(&format!(
            "<li>Page {}: <q>{}</q></li>",
            highlight.page,
            escape_html(quote)
        ));
    }
    out.push_str("</ul><h2>Notes</h2>");
    for note in notes {
        out.push_str(&format!(
            "<article><h3>Page {}</h3><p>{}</p>",
            note.page,
            escape_html(&note.text).replace('\n', "<br>\n")
        ));
        if !note.replies.is_empty() {
            out.push_str("<ul>");
            for reply in &note.replies {
                out.push_str(&format!("<li>{}</li>", escape_html(&reply.text)));
            }
            out.push_str("</ul>");
        }
        out.push_str("</article>");
    }
    out.push_str("</body></html>");
    out
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_export_escapes_book_and_note_text_and_includes_replies() {
        let notes = [Postit::new(2, "<script>".to_owned(), None)];
        let html = html("A & B", &notes, &[], &[]);
        assert!(html.contains("A &amp; B"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn markdown_export_includes_all_annotation_types() {
        let notes = [Postit::new(2, "Note body".to_owned(), None)];
        let highlights = [Highlight {
            page: 1,
            start: 0,
            end: 4,
            anchor: Some(crate::persist::TextAnchor {
                quote: "word".to_owned(),
                before: String::new(),
                after: String::new(),
            }),
            created_at: 0,
        }];
        let bookmarks = [Bookmark {
            page: 3,
            anchor: None,
        }];
        let md = markdown("Book", &notes, &highlights, &bookmarks);
        assert!(md.contains("Page 3"));
        assert!(md.contains("\"word\""));
        assert!(md.contains("Note body"));
    }
}
