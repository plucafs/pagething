use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading {
        level: u8,
        spans: Vec<Span>,
    },
    Para(Vec<Span>),
    Quote(Vec<Block>),
    List {
        ordered: bool,
        items: Vec<Vec<Block>>,
    },
    Image {
        path: String,
        alt: String,
    },
    Rule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Inline {
    Text(Span),
    Image { path: String, alt: String },
}

struct Cursor<'a> {
    events: Vec<Event<'a>>,
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<Event<'a>> {
        self.events.get(self.pos).cloned()
    }

    fn next(&mut self) -> Option<Event<'a>> {
        let event = self.events.get(self.pos).cloned();
        if event.is_some() {
            self.pos += 1;
        }
        event
    }

    fn back_up(&mut self) {
        self.pos -= 1;
    }
}

#[derive(Default, Clone, Copy)]
struct Style {
    bold: bool,
    italic: bool,
    code: bool,
}

pub fn parse_md(md: &str) -> Vec<Block> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    let events: Vec<Event<'_>> = Parser::new_ext(md, options).collect();
    let mut cursor = Cursor { events, pos: 0 };
    parse_blocks(&mut cursor, None)
}

fn parse_blocks(cursor: &mut Cursor<'_>, stop_at: Option<TagEnd>) -> Vec<Block> {
    let mut blocks = Vec::new();
    while let Some(event) = cursor.peek() {
        match event {
            Event::Start(tag) => {
                let tag = tag.clone();
                let end = matching_end(&tag);
                cursor.next();
                match tag {
                    Tag::Heading { level, .. } => {
                        let inlines = parse_inlines(cursor, Some(end));
                        blocks.push(Block::Heading {
                            level: level as u8,
                            spans: text_spans(inlines),
                        });
                    }
                    Tag::Paragraph => {
                        let inlines = parse_inlines(cursor, Some(end));
                        blocks.extend(blocks_from_inlines(inlines));
                    }
                    Tag::BlockQuote(_) => {
                        blocks.push(Block::Quote(parse_blocks(cursor, Some(end))));
                    }
                    Tag::List(ordered) => {
                        let mut items = Vec::new();
                        while let Some(Event::Start(Tag::Item)) = cursor.peek() {
                            cursor.next();
                            items.push(parse_blocks(cursor, Some(TagEnd::Item)));
                        }
                        blocks.push(Block::List {
                            ordered: ordered.is_some(),
                            items,
                        });
                    }
                    Tag::CodeBlock(_) => {
                        let mut text = String::new();
                        while let Some(ev) = cursor.next() {
                            match ev {
                                Event::Text(t) => text.push_str(&t),
                                Event::End(TagEnd::CodeBlock) => break,
                                _ => {}
                            }
                        }
                        blocks.push(Block::Para(vec![Span {
                            text,
                            code: true,
                            ..Default::default()
                        }]));
                    }
                    Tag::HtmlBlock => skip_to_end(cursor, TagEnd::HtmlBlock),
                    _ => skip_unknown_container(cursor),
                }
            }
            Event::Rule => {
                cursor.next();
                blocks.push(Block::Rule);
            }
            Event::Text(_) | Event::Code(_) | Event::SoftBreak | Event::HardBreak => {
                let inlines = parse_inlines(cursor, None);
                blocks.extend(blocks_from_inlines(inlines));
            }
            Event::End(end) => {
                cursor.next();
                if Some(end) == stop_at {
                    return blocks;
                }
            }
            _ => {
                cursor.next();
            }
        }
    }
    blocks
}

fn parse_inlines(cursor: &mut Cursor<'_>, stop_at: Option<TagEnd>) -> Vec<Inline> {
    let mut inlines = Vec::new();
    let mut style = Style::default();
    let mut text = String::new();

    macro_rules! flush {
        () => {
            if !text.is_empty() {
                inlines.push(Inline::Text(Span {
                    text: std::mem::take(&mut text),
                    bold: style.bold,
                    italic: style.italic,
                    code: style.code,
                }));
            }
        };
    }

    while let Some(event) = cursor.next() {
        match event {
            Event::Text(t) => text.push_str(&t),
            Event::Code(code) => {
                flush!();
                inlines.push(Inline::Text(Span {
                    text: code.to_string(),
                    code: true,
                    ..Default::default()
                }));
            }
            Event::SoftBreak | Event::HardBreak => text.push(' '),
            Event::Start(tag) if is_inline_start(&tag) => match tag {
                Tag::Strong => {
                    flush!();
                    style.bold = true;
                }
                Tag::Emphasis => {
                    flush!();
                    style.italic = true;
                }
                Tag::Image { dest_url, .. } => {
                    flush!();
                    let path = dest_url.to_string();
                    let alt = collect_alt_text(cursor);
                    inlines.push(Inline::Image { path, alt });
                }
                _ => {}
            },
            Event::End(tag) if is_inline_end(&tag) => match tag {
                TagEnd::Strong => {
                    flush!();
                    style.bold = false;
                }
                TagEnd::Emphasis => {
                    flush!();
                    style.italic = false;
                }
                _ => {
                    if Some(tag) == stop_at {
                        flush!();
                        return inlines;
                    }
                }
            },
            Event::End(tag) => {
                // Block-level end: consume it only if it is our stopper.
                if Some(tag) == stop_at {
                    flush!();
                    return inlines;
                }
                cursor.back_up();
                flush!();
                return inlines;
            }
            Event::Start(_) => {
                // Block-level start: leave it for parse_blocks.
                cursor.back_up();
                flush!();
                return inlines;
            }
            _ => {}
        }
    }
    flush!();
    inlines
}

/// Image alt text is the run of inline events between Start(Image) and End(Image).
fn collect_alt_text(cursor: &mut Cursor<'_>) -> String {
    let mut alt = String::new();
    while let Some(event) = cursor.next() {
        match event {
            Event::Text(t) | Event::Code(t) => alt.push_str(&t),
            Event::End(TagEnd::Image) => break,
            Event::Start(_) => {}
            _ => break,
        }
    }
    alt
}

fn text_spans(inlines: Vec<Inline>) -> Vec<Span> {
    inlines
        .into_iter()
        .map(|inline| match inline {
            Inline::Text(span) => span,
            Inline::Image { alt, .. } => Span {
                text: alt,
                ..Default::default()
            },
        })
        .collect()
}

/// Group a paragraph's inlines into blocks: consecutive text -> Para, images split it.
fn blocks_from_inlines(inlines: Vec<Inline>) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut pending: Vec<Span> = Vec::new();
    for inline in inlines {
        match inline {
            Inline::Text(span) => pending.push(span),
            Inline::Image { path, alt } => {
                if !pending.is_empty() {
                    blocks.push(Block::Para(std::mem::take(&mut pending)));
                }
                blocks.push(Block::Image { path, alt });
            }
        }
    }
    if !pending.is_empty() {
        blocks.push(Block::Para(pending));
    }
    blocks
}

fn is_inline_start(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::Strong
            | Tag::Emphasis
            | Tag::Strikethrough
            | Tag::Link { .. }
            | Tag::Image { .. }
            | Tag::Superscript
            | Tag::Subscript
    )
}

fn is_inline_end(tag: &TagEnd) -> bool {
    matches!(
        tag,
        TagEnd::Strong
            | TagEnd::Emphasis
            | TagEnd::Strikethrough
            | TagEnd::Link
            | TagEnd::Image
            | TagEnd::Superscript
            | TagEnd::Subscript
    )
}

fn skip_to_end(cursor: &mut Cursor<'_>, end: TagEnd) {
    let mut depth = 1usize;
    while depth > 0 {
        match cursor.next() {
            Some(Event::Start(_)) => depth += 1,
            Some(Event::End(e)) if e == end => depth -= 1,
            Some(Event::End(_)) => {}
            _ => break,
        }
    }
}

fn skip_unknown_container(cursor: &mut Cursor<'_>) {
    let mut depth = 1usize;
    while depth > 0 {
        match cursor.next() {
            Some(Event::Start(_)) => depth += 1,
            Some(Event::End(_)) => depth -= 1,
            _ => break,
        }
    }
}

fn matching_end(tag: &Tag<'_>) -> TagEnd {
    match tag {
        Tag::Paragraph => TagEnd::Paragraph,
        Tag::Heading { level, .. } => TagEnd::Heading(*level),
        Tag::BlockQuote(_) => TagEnd::BlockQuote(None),
        Tag::CodeBlock(_) => TagEnd::CodeBlock,
        Tag::HtmlBlock => TagEnd::HtmlBlock,
        Tag::List(ordered) => TagEnd::List(ordered.is_some()),
        Tag::Item => TagEnd::Item,
        Tag::FootnoteDefinition(_) => TagEnd::FootnoteDefinition,
        Tag::DefinitionList => TagEnd::DefinitionList,
        Tag::DefinitionListTitle => TagEnd::DefinitionListTitle,
        Tag::DefinitionListDefinition => TagEnd::DefinitionListDefinition,
        Tag::Table(_) => TagEnd::Table,
        Tag::TableHead => TagEnd::TableHead,
        Tag::TableRow => TagEnd::TableRow,
        Tag::TableCell => TagEnd::TableCell,
        Tag::Emphasis => TagEnd::Emphasis,
        Tag::Strong => TagEnd::Strong,
        Tag::Strikethrough => TagEnd::Strikethrough,
        Tag::Superscript => TagEnd::Superscript,
        Tag::Subscript => TagEnd::Subscript,
        Tag::Link { .. } => TagEnd::Link,
        Tag::Image { .. } => TagEnd::Image,
        Tag::MetadataBlock(kind) => TagEnd::MetadataBlock(*kind),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_heading_para_and_styles() {
        let blocks = parse_md("## Chapter I\n\nIt is a *truth* universally **acknowledged**.\n");
        assert_eq!(blocks.len(), 2);
        assert_eq!(
            blocks[0],
            Block::Heading {
                level: 2,
                spans: vec![Span {
                    text: "Chapter I".into(),
                    ..Default::default()
                }]
            }
        );
        match &blocks[1] {
            Block::Para(spans) => {
                let text: String = spans.iter().map(|s| s.text.as_str()).collect();
                assert_eq!(text, "It is a truth universally acknowledged.");
                assert!(spans[1].italic && spans[1].text == "truth");
                assert!(spans[3].bold && spans[3].text == "acknowledged");
            }
            other => panic!("expected para, got {other:?}"),
        }
    }

    #[test]
    fn parses_list_quote_rule() {
        let blocks = parse_md("- one\n- two\n\n> quoted\n\n---\n");
        assert_eq!(blocks.len(), 3);
        match &blocks[0] {
            Block::List { ordered, items } => {
                assert!(!ordered);
                assert_eq!(items.len(), 2);
            }
            other => panic!("expected list, got {other:?}"),
        }
        assert!(matches!(&blocks[1], Block::Quote(inner) if !inner.is_empty()));
        assert_eq!(blocks[2], Block::Rule);
    }

    #[test]
    fn parses_nested_list() {
        let blocks = parse_md("- outer\n  - inner\n");
        match &blocks[0] {
            Block::List { items, .. } => {
                assert_eq!(items.len(), 1);
                assert!(items[0].iter().any(|b| matches!(b, Block::List { .. })));
            }
            other => panic!("expected list, got {other:?}"),
        }
    }

    #[test]
    fn parses_image_splitting_paragraph() {
        let blocks = parse_md("before\n\n![alt text](/EPUB/images/a.png)\n\nafter");
        assert_eq!(blocks.len(), 3);
        assert_eq!(
            blocks[1],
            Block::Image {
                path: "/EPUB/images/a.png".into(),
                alt: "alt text".into()
            }
        );
    }

    #[test]
    fn parses_inline_image_in_paragraph() {
        let blocks = parse_md("see ![diagram](/img/d.png) here");
        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[1], Block::Image { path, .. } if path == "/img/d.png"));
    }
}
