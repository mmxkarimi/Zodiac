//! HTML to Markdown, built on the `html-to-markdown-rs` crate.
//!
//! Cargo.toml (needs Rust 1.88 or newer):
//!
//! ```toml
//! [dependencies]
//! html-to-markdown-rs = { version = "=3.18.6", features = ["visitor"] }
//! ```
//!
//! What this file does on top of the crate:
//! - links are removed, only the link text stays
//! - images are left out
//! - citation marks like `<sup class="reference">[1]</sup>` are dropped
//! - metadata work is turned off, because only the text is needed

use html_to_markdown_rs::visitor::{HtmlVisitor, NodeContext, VisitResult, VisitorHandle};
use html_to_markdown_rs::{convert, ConversionOptions, HeadingStyle};
use std::sync::{Arc, Mutex};

// ------------------------------------------------------------------
// Public API
// ------------------------------------------------------------------

/// Converts HTML to Markdown. Returns an empty string for empty or bad input.
pub fn html_to_markdown(html: &str) -> String {
    if html.trim().is_empty() {
        return String::new();
    }

    let visitor: VisitorHandle = Arc::new(Mutex::new(DropLinks));

    let options = ConversionOptions {
        heading_style: HeadingStyle::Atx,
        bullets: "-".to_string(),
        list_indent_width: 2,
        autolinks: false,
        skip_images: true,
        extract_metadata: false,
        exclude_selectors: vec!["sup.reference".to_string()],
        visitor: Some(visitor),
        ..Default::default()
    };

    match convert(html, Some(options)) {
        Ok(result) => {
            let md = result.content.unwrap_or_default();
            let md = md.trim();
            if md.is_empty() {
                String::new()
            } else {
                format!("{}\n", md)
            }
        }
        Err(_) => String::new(),
    }
}

/// Reads `&amp;`, `&#65;`, `&#x41;` style entities. Unknown ones stay as they are.
pub fn decode_html_entities(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '&' {
            // Look for the closing ';' within a short distance.
            let end = chars[i + 1..]
                .iter()
                .take(12)
                .position(|&c| c == ';')
                .map(|p| p + i + 1);

            if let Some(end) = end {
                let name: String = chars[i + 1..end].iter().collect();
                if let Some(decoded) = decode_one(&name) {
                    out.push(decoded);
                    i = end + 1;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

// ------------------------------------------------------------------
// Link remover
// ------------------------------------------------------------------

/// Replaces every link with its text.
#[derive(Debug)]
struct DropLinks;

impl HtmlVisitor for DropLinks {
    fn visit_link(
        &mut self,
        _ctx: &NodeContext<'_>,
        _href: &str,
        text: &str,
        _title: Option<&str>,
    ) -> VisitResult {
        VisitResult::Custom(text.to_string())
    }
}

// ------------------------------------------------------------------
// Entity helper
// ------------------------------------------------------------------

fn decode_one(name: &str) -> Option<char> {
    if let Some(num) = name.strip_prefix('#') {
        let code = if let Some(hex) = num.strip_prefix('x').or_else(|| num.strip_prefix('X')) {
            u32::from_str_radix(hex, 16).ok()?
        } else {
            num.parse::<u32>().ok()?
        };
        return char::from_u32(code);
    }
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{00A0}',
        _ => return None,
    })
}

// ------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        assert_eq!(html_to_markdown("   "), "");
    }

    #[test]
    fn test_html_strip_scripts_and_styles() {
        let html = "<html><head><script>alert(1);</script><style>body { color: red; }</style></head><body><h1>Title</h1><p>Hello &amp; welcome</p></body></html>";
        let md = html_to_markdown(html);
        assert!(!md.contains("alert"));
        assert!(!md.contains("color: red"));
        assert!(md.contains("# Title"));
        assert!(md.contains("Hello & welcome"));
    }

    #[test]
    fn test_html_formatting() {
        let html = "<p>Text with <b>bold</b> and <i>italic</i> and <code>code</code></p><ul><li>One</li><li>Two</li></ul>";
        let md = html_to_markdown(html);
        assert!(md.contains("**bold**"));
        assert!(md.contains("code"));
        assert!(md.contains("- One"));
        assert!(md.contains("- Two"));
    }

    #[test]
    fn test_utf8_non_ascii() {
        let html = "<p>داستان <b>رستم و سهراب</b> در شاهنامه فردوسی</p>";
        let md = html_to_markdown(html);
        assert!(md.contains("**رستم و سهراب**"));
    }

    #[test]
    fn test_links_and_images_removed() {
        let html = r##"<p>See <a href="https://a.com/x" title="T">the site</a>, <a href="https://b.com">https://b.com</a>, <a href="#c1">note</a> <img src="p.png" alt="pic"></p>"##;
        let md = html_to_markdown(html);
        assert!(md.contains("the site"));
        assert!(!md.contains("]("));
        assert!(!md.contains("a.com"));
        assert!(!md.contains("p.png"));
    }

    #[test]
    fn test_citation_marks_dropped() {
        let html = r##"<p>Tux<sup class="reference"><a href="#c1">[1]</a></sup> is a penguin</p>"##;
        let md = html_to_markdown(html);
        assert!(md.contains("Tux is a penguin"));
        assert!(!md.contains("[1]"));
    }

    #[test]
    fn test_entities() {
        assert_eq!(
            decode_html_entities("a &lt; b &#65; &#x42; &bogus;"),
            "a < b A B &bogus;"
        );
    }
}