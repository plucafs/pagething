use crate::book::paginate::Page;

#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    pub page: usize,
    pub start: usize,
    pub end: usize,
    pub snippet: String,
}

/// Case-insensitive (ASCII) substring search over every page's plain text.
/// Char offsets. Pages keep their own offset space.
pub fn search(pages: &[Page], query: &str) -> Vec<Hit> {
    let needle = query.to_ascii_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let needle_chars = needle.chars().count();
    let mut hits = Vec::new();
    for (pi, page) in pages.iter().enumerate() {
        if page.plain_text.is_empty() {
            continue;
        }
        let hay = page.plain_text.to_ascii_lowercase();
        let mut from = 0usize;
        while let Some(rel) = hay[from..].find(&needle) {
            let byte_start = from + rel;
            let start = hay[..byte_start].chars().count();
            let end = start + needle_chars;
            hits.push(Hit {
                page: pi,
                start,
                end,
                snippet: snippet(&page.plain_text, start, end),
            });
            from = byte_start + hay[byte_start..].chars().next().map_or(0, char::len_utf8);
        }
    }
    hits
}

const SNIPPET_CONTEXT: usize = 30;

pub fn snippet(text: &str, start: usize, end: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let start = start.min(chars.len());
    let end = end.clamp(start, chars.len());
    let from = start.saturating_sub(SNIPPET_CONTEXT);
    let to = (end + SNIPPET_CONTEXT).min(chars.len());
    let middle: String = chars[from..to].iter().collect();
    let collapsed = middle.split_whitespace().collect::<Vec<_>>().join(" ");
    let prefix = if from > 0 { "…" } else { "" };
    let suffix = if to < chars.len() { "…" } else { "" };
    format!("{prefix}{collapsed}{suffix}")
}

/// Char-range intersections of `[a, b)` with each of `segments` (sorted by start).
/// Returns `(segment index, range start, range end)` clipped to the query range.
pub fn segment_ranges(
    segments: &[crate::book::paginate::TextSegment],
    a: usize,
    b: usize,
) -> Vec<(usize, usize, usize)> {
    let mut out = Vec::new();
    if a >= b {
        return out;
    }
    for (i, seg) in segments.iter().enumerate() {
        let start = seg.start.max(a);
        let end = seg.end.min(b);
        if start < end {
            out.push((i, start, end));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::paginate::{Page, PageKind, TextSegment};

    fn page(text: &str) -> Page {
        let chars = text.chars().count();
        Page {
            kind: PageKind::Content,
            items: Vec::new(),
            plain_text: text.to_owned(),
            segments: vec![TextSegment {
                block: 0,
                start: 0,
                end: chars,
            }],
        }
    }

    #[test]
    fn finds_case_insensitive_matches_with_offsets() {
        let pages = [page("Hello world. hello again.")];
        let hits = search(&pages, "hello");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].page, 0);
        assert_eq!(hits[0].start, 0);
        assert_eq!(hits[0].end, 5);
        assert_eq!(hits[1].start, 13);
        assert_eq!(hits[1].end, 18);
    }

    #[test]
    fn empty_query_and_no_match_yield_nothing() {
        let pages = [page("abc")];
        assert!(search(&pages, "").is_empty());
        assert!(search(&pages, "zzz").is_empty());
    }

    #[test]
    fn spans_pages_and_snippets_collapse_whitespace() {
        let pages = [
            page("nothing here"),
            page(&format!("{} match {}", "word ".repeat(40), "x ".repeat(40))),
        ];
        let hits = search(&pages, "match");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].page, 1);
        assert!(!hits[0].snippet.contains('\n'));
        assert!(hits[0].snippet.starts_with('…'));
        assert!(hits[0].snippet.ends_with('…'));
    }

    #[test]
    fn char_offsets_not_bytes() {
        let pages = [page("héllo ünïcode")];
        let hits = search(&pages, "ünïcode");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].start, 6);
        assert_eq!(hits[0].end, 13);
    }

    #[test]
    fn segment_ranges_clip_to_query() {
        use crate::book::paginate::TextSegment;
        let segments = [
            TextSegment {
                block: 0,
                start: 0,
                end: 10,
            },
            TextSegment {
                block: 1,
                start: 11,
                end: 20,
            },
        ];
        assert_eq!(
            segment_ranges(&segments, 8, 15),
            vec![(0, 8, 10), (1, 11, 15)]
        );
        assert!(segment_ranges(&segments, 5, 5).is_empty());
        assert!(segment_ranges(&segments, 0, 100).len() == 2);
    }
}
