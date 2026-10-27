use std::io;

use htmd::HtmlToMarkdown;

use super::load::{Chapter, resolve_href};

pub fn xhtml_to_md(xhtml: &str) -> io::Result<String> {
    HtmlToMarkdown::builder()
        .skip_tags(vec!["script", "style", "head"])
        .build()
        .convert(xhtml)
}

/// Convert a chapter and resolve image hrefs against the chapter location.
pub fn chapter_to_md(chapter: &Chapter) -> io::Result<String> {
    let md = xhtml_to_md(&chapter.xhtml)?;
    Ok(rewrite_image_hrefs(&md, &chapter.href))
}

/// Rewrite `![alt](src)` hrefs to normalized EPUB resource paths.
fn rewrite_image_hrefs(md: &str, chapter_href: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut rest = md;
    while let Some(start) = rest.find("![") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(alt_end) = after.find("](") else {
            out.push_str("![");
            rest = after;
            continue;
        };
        let alt = &after[..alt_end];
        let after = &after[alt_end + 2..];
        let Some(close) = after.find(')') else {
            out.push_str("![");
            rest = after;
            continue;
        };
        let raw_src = &after[..close];
        let resolved = resolve_href(chapter_href, raw_src);
        out.push_str(&format!("![{alt}]({resolved})"));
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_basic_xhtml() {
        let xhtml = r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml">
<head><title>t</title><style>p{margin:0}</style></head>
<body><h2>Chapter I</h2><p>It is a <em>truth</em> universally <strong>acknowledged</strong>.</p>
<img src="../images/fig1.png" alt="Fig 1"/></body></html>"#;
        let chapter = Chapter {
            href: "/EPUB/text/ch1.xhtml".to_string(),
            xhtml: xhtml.to_string(),
        };
        let md = chapter_to_md(&chapter).unwrap();
        assert!(md.contains("## Chapter I"));
        assert!(md.contains("*truth*"));
        assert!(md.contains("**acknowledged**"));
        assert!(md.contains("![Fig 1](/EPUB/images/fig1.png)"));
        assert!(!md.contains("margin:0"));
    }
}
