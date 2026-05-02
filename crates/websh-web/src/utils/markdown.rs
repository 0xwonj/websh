//! HTML and Markdown rendering utilities.
//!
//! Provides safe HTML rendering boundaries with XSS protection.

use std::collections::{HashMap, HashSet};

use comrak::{Options, markdown_to_html as comrak_markdown_to_html};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RenderedMarkdown {
    pub html: String,
    pub has_math: bool,
    /// In-document outline (h2 / h3 only) extracted from the rendered HTML.
    /// Empty for inputs with no qualifying headings or for non-markdown
    /// HTML inputs that lack `id` attributes on their headings.
    pub outline: Vec<HeadingEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadingEntry {
    pub level: u8,
    pub text: String,
    pub id: String,
}

/// Sanitize untrusted HTML before rendering it with `inner_html`.
pub fn sanitize_html(html: &str) -> String {
    let mut builder = ammonia::Builder::empty();
    builder
        .tags(markdown_tags())
        .tag_attributes(markdown_tag_attributes())
        .generic_attributes(HashSet::from(["lang", "title"]))
        .url_schemes(HashSet::from(["http", "https", "mailto"]))
        .link_rel(Some("noopener noreferrer"));
    builder.add_tag_attribute_values("input", "type", &["checkbox"]);
    builder.add_tag_attribute_values("span", "data-math-style", &["inline", "display"]);
    builder.clean(html).to_string()
}

/// Convert markdown content to sanitized HTML plus hydration metadata.
pub fn render_markdown(markdown: &str) -> RenderedMarkdown {
    let html = comrak_markdown_to_html(markdown, &markdown_options());
    rendered_from_html(sanitize_html(&html))
}

/// Convert a single inline markdown fragment to sanitized HTML plus hydration metadata.
pub fn render_inline_markdown(markdown: &str) -> RenderedMarkdown {
    let rendered = render_markdown(markdown);
    rendered_from_html(strip_paragraph_wrapper(&rendered.html).to_string())
}

pub fn rendered_from_html(html: String) -> RenderedMarkdown {
    let has_math =
        html.contains("data-math-style=\"inline\"") || html.contains("data-math-style=\"display\"");
    let outline = extract_outline(&html);
    RenderedMarkdown {
        html,
        has_math,
        outline,
    }
}

/// Walk a sanitized HTML body for `<h2>` / `<h3>` blocks and capture the
/// (level, anchor id, visible text) triple for each. Comrak emits the
/// heading id on an inner self-link `<a id="...">`; we tolerate that
/// shape and a bare `<h2 id="...">` as fallback.
///
/// The text strips inline markup (`<code>`, `<strong>`, …) and decodes
/// the entities ammonia produces (`&amp;`, `&lt;`, `&gt;`, `&quot;`,
/// `&#39;`, `&apos;`, plus numeric ampersand-escapes).
fn extract_outline(html: &str) -> Vec<HeadingEntry> {
    let mut entries = Vec::new();
    let bytes = html.as_bytes();
    let mut cursor = 0;

    while cursor < bytes.len() {
        let Some(rel) = html[cursor..].find("<h") else {
            break;
        };
        let tag_start = cursor + rel;
        let level_pos = tag_start + 2;
        let level = match bytes.get(level_pos) {
            Some(b'2') => 2u8,
            Some(b'3') => 3u8,
            _ => {
                cursor = level_pos;
                continue;
            }
        };

        // Confirm tag boundary — `<h2x` should be skipped.
        let after_level = level_pos + 1;
        match bytes.get(after_level) {
            Some(b' ' | b'\t' | b'\n' | b'>') => {}
            _ => {
                cursor = after_level;
                continue;
            }
        }

        // Find the matching closing tag.
        let close_tag = if level == 2 { "</h2>" } else { "</h3>" };
        let Some(close_rel) = html[after_level..].find(close_tag) else {
            break;
        };
        let block_end = after_level + close_rel;
        let block = &html[tag_start..block_end];

        // The id attribute may sit on the opening `<h*>` itself or on the
        // inner self-link Comrak emits; either is fine.
        let Some(id) = find_id_attr(block) else {
            cursor = block_end + close_tag.len();
            continue;
        };

        // Locate the body content — skip past the opening `<h*…>` tag
        // and (when present) the empty inner anchor Comrak prepends.
        let after_open = match block.find('>') {
            Some(p) => p + 1,
            None => {
                cursor = block_end + close_tag.len();
                continue;
            }
        };
        let body = &block[after_open..];
        let body = body.strip_prefix("<a").map_or(body, |rest| {
            // Skip the closing `</a>` of the opening anchor (if any).
            rest.find("</a>")
                .map(|p| &rest[p + "</a>".len()..])
                .unwrap_or(body)
        });

        let text = decode_entities(&strip_tags(body)).trim().to_string();
        if !text.is_empty() {
            entries.push(HeadingEntry {
                level,
                text,
                id: id.to_string(),
            });
        }

        cursor = block_end + close_tag.len();
    }

    entries
}

fn find_id_attr(block: &str) -> Option<&str> {
    let needle = "id=\"";
    let mut cursor = 0;
    while let Some(rel) = block[cursor..].find(needle) {
        let attr_start = cursor + rel;
        // The id= must be preceded by whitespace to be a real attribute,
        // not the tail of some other token.
        let preceded_by_ws =
            attr_start == 0 || matches!(block.as_bytes()[attr_start - 1], b' ' | b'\t' | b'\n');
        let value_start = attr_start + needle.len();
        let value_end_rel = block[value_start..].find('"')?;
        if preceded_by_ws {
            return Some(&block[value_start..value_start + value_end_rel]);
        }
        cursor = value_start + value_end_rel + 1;
    }
    None
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            while i < bytes.len() && bytes[i] != b'>' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'&'
            && let Some(end_rel) = s[i..].find(';')
        {
            let entity = &s[i..i + end_rel + 1];
            if let Some(decoded) = decode_named_entity(entity) {
                out.push_str(decoded);
                i += end_rel + 1;
                continue;
            }
            if let Some(decoded) = decode_numeric_entity(entity) {
                out.push(decoded);
                i += end_rel + 1;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn decode_named_entity(entity: &str) -> Option<&'static str> {
    match entity {
        "&amp;" => Some("&"),
        "&lt;" => Some("<"),
        "&gt;" => Some(">"),
        "&quot;" => Some("\""),
        "&apos;" => Some("'"),
        "&nbsp;" => Some("\u{00A0}"),
        _ => None,
    }
}

fn decode_numeric_entity(entity: &str) -> Option<char> {
    let body = entity.strip_prefix("&#")?.strip_suffix(';')?;
    let code: u32 = if let Some(hex) = body.strip_prefix(['x', 'X']) {
        u32::from_str_radix(hex, 16).ok()?
    } else {
        body.parse().ok()?
    };
    char::from_u32(code)
}

fn markdown_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.autolink = true;
    options.extension.front_matter_delimiter = Some("---".to_string());
    options.extension.header_id_prefix = Some(String::new());
    options.extension.math_dollars = true;
    options.extension.math_code = true;
    options.render.r#unsafe = false;
    options
}

fn markdown_tags() -> HashSet<&'static str> {
    HashSet::from([
        "a",
        "blockquote",
        "br",
        "caption",
        "code",
        "col",
        "colgroup",
        "del",
        "em",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "hr",
        "img",
        "input",
        "li",
        "ol",
        "p",
        "pre",
        "section",
        "span",
        "strong",
        "sup",
        "table",
        "tbody",
        "td",
        "th",
        "thead",
        "tr",
        "ul",
    ])
}

fn markdown_tag_attributes() -> HashMap<&'static str, HashSet<&'static str>> {
    let mut attrs = HashMap::new();
    attrs.insert(
        "a",
        HashSet::from([
            "aria-label",
            "data-footnote-backref",
            "data-footnote-ref",
            "href",
            "id",
            "title",
        ]),
    );
    attrs.insert("col", HashSet::from(["span"]));
    attrs.insert("h1", HashSet::from(["id"]));
    attrs.insert("h2", HashSet::from(["id"]));
    attrs.insert("h3", HashSet::from(["id"]));
    attrs.insert("h4", HashSet::from(["id"]));
    attrs.insert("h5", HashSet::from(["id"]));
    attrs.insert("h6", HashSet::from(["id"]));
    attrs.insert(
        "img",
        HashSet::from(["alt", "height", "src", "title", "width"]),
    );
    attrs.insert("input", HashSet::from(["checked", "disabled", "type"]));
    attrs.insert("li", HashSet::from(["id"]));
    attrs.insert("ol", HashSet::from(["start"]));
    attrs.insert("section", HashSet::from(["data-footnotes"]));
    attrs.insert("span", HashSet::from(["data-math-style"]));
    attrs.insert("td", HashSet::from(["colspan", "rowspan"]));
    attrs.insert("th", HashSet::from(["colspan", "rowspan", "scope"]));
    attrs
}

fn strip_paragraph_wrapper(html: &str) -> &str {
    html.strip_prefix("<p>")
        .and_then(|inner| {
            inner
                .strip_suffix("</p>\n")
                .or_else(|| inner.strip_suffix("</p>"))
        })
        .unwrap_or(html)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_namespace = katex, js_name = render, catch)]
    fn katex_render(
        tex: &str,
        element: &web_sys::Element,
        options: &wasm_bindgen::JsValue,
    ) -> Result<(), wasm_bindgen::JsValue>;
}

#[cfg(target_arch = "wasm32")]
pub fn hydrate_math(root: &web_sys::Element) {
    use wasm_bindgen::JsCast;

    let Ok(nodes) = root.query_selector_all("[data-math-style]:not([data-katex-rendered])") else {
        return;
    };

    for index in 0..nodes.length() {
        let Some(node) = nodes.item(index) else {
            continue;
        };
        let Ok(element) = node.dyn_into::<web_sys::Element>() else {
            continue;
        };
        let tex = element.text_content().unwrap_or_default();
        if tex.trim().is_empty() {
            continue;
        }

        let display = element
            .get_attribute("data-math-style")
            .as_deref()
            .is_some_and(|style| style == "display");
        let options = katex_options(display);

        match katex_render(&tex, &element, &options) {
            Ok(()) => {
                let _ = element.set_attribute("data-katex-rendered", "true");
            }
            Err(error) => web_sys::console::warn_1(&error),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn hydrate_math(_root: &web_sys::Element) {}

#[cfg(target_arch = "wasm32")]
fn katex_options(display_mode: bool) -> wasm_bindgen::JsValue {
    use wasm_bindgen::JsValue;

    let options = js_sys::Object::new();
    set_js_option(&options, "displayMode", JsValue::from_bool(display_mode));
    set_js_option(&options, "throwOnError", JsValue::from_bool(false));
    set_js_option(&options, "trust", JsValue::from_bool(false));
    set_js_option(&options, "strict", JsValue::from_str("warn"));
    set_js_option(&options, "output", JsValue::from_str("htmlAndMathml"));
    set_js_option(&options, "maxSize", JsValue::from_f64(12.0));
    set_js_option(&options, "maxExpand", JsValue::from_f64(500.0));
    options.into()
}

#[cfg(target_arch = "wasm32")]
fn set_js_option(options: &js_sys::Object, key: &str, value: wasm_bindgen::JsValue) {
    let _ = js_sys::Reflect::set(options, &wasm_bindgen::JsValue::from_str(key), &value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_html_removes_scripts_and_event_handlers() {
        let html = r#"<img src="x" onerror="alert(1)"><script>alert(2)</script>"#;
        let sanitized = sanitize_html(html);
        assert!(!sanitized.contains("onerror"));
        assert!(!sanitized.contains("<script"));
    }

    #[test]
    fn render_markdown_strips_frontmatter() {
        let rendered = render_markdown("---\ndate: 2026-04-26\ntags: [math]\n---\n\n# Body\n");
        assert!(rendered.html.contains(r##"<a href="#body""##));
        assert!(rendered.html.contains(r#"id="body""#));
        assert!(!rendered.html.contains("date:"));
        assert!(!rendered.html.contains("tags:"));
    }

    #[test]
    fn render_markdown_keeps_safe_links() {
        let html = render_markdown("Writing [tabula](/#/papers/tabula).").html;
        assert!(
            html.contains(r#"<a href="/#/papers/tabula" rel="noopener noreferrer">tabula</a>"#)
        );
    }

    #[test]
    fn render_inline_markdown_keeps_links_without_paragraph_wrapper() {
        let html = render_inline_markdown("Writing [tabula](/#/papers/tabula).").html;
        assert!(
            html.contains(r#"<a href="/#/papers/tabula" rel="noopener noreferrer">tabula</a>"#)
        );
        assert!(!html.starts_with("<p>"));
    }

    #[test]
    fn render_markdown_omits_raw_html_and_scripts() {
        let rendered = render_markdown(r#"<script>alert(1)</script><img src=x onerror=alert(2)>"#);
        assert!(!rendered.html.contains("<script"));
        assert!(!rendered.html.contains("onerror"));
    }

    #[test]
    fn render_markdown_outputs_inline_math_placeholder() {
        let rendered = render_markdown("$E = mc^2$");
        assert!(rendered.has_math);
        assert!(rendered.html.contains(r#"data-math-style="inline""#));
        assert!(rendered.html.contains("E = mc^2"));
    }

    #[test]
    fn render_markdown_outputs_display_math_placeholder() {
        let rendered = render_markdown("$$x^2$$");
        assert!(rendered.has_math);
        assert!(rendered.html.contains(r#"data-math-style="display""#));
        assert!(rendered.html.contains("x^2"));
    }

    #[test]
    fn render_markdown_keeps_escaped_dollars_literal() {
        let rendered = render_markdown("Cost is \\$5.");
        assert!(!rendered.has_math);
        assert!(rendered.html.contains("Cost is $5."));
    }

    #[test]
    fn outline_collects_h2_h3_in_document_order() {
        let md = "# Title\n\n## Section A\n\nfoo\n\n### Sub A1\n\nbar\n\n## Section B\n";
        let rendered = render_markdown(md);
        let outline = &rendered.outline;
        assert_eq!(outline.len(), 3, "{:?}", outline);
        assert_eq!(outline[0].level, 2);
        assert_eq!(outline[0].text, "Section A");
        assert_eq!(outline[1].level, 3);
        assert_eq!(outline[1].text, "Sub A1");
        assert_eq!(outline[2].level, 2);
        assert_eq!(outline[2].text, "Section B");
    }

    #[test]
    fn outline_skips_h1_and_h4_through_h6() {
        let md = "# H1\n\n## H2\n\n### H3\n\n#### H4\n\n##### H5\n\n###### H6\n";
        let rendered = render_markdown(md);
        let levels: Vec<u8> = rendered.outline.iter().map(|e| e.level).collect();
        assert_eq!(levels, vec![2, 3]);
    }

    #[test]
    fn outline_empty_for_no_headings() {
        let rendered = render_markdown("Just a paragraph with no headings.\n");
        assert!(rendered.outline.is_empty());
    }

    #[test]
    fn outline_strips_inline_formatting_from_text() {
        let md = "## With **bold** and `code` and *em*\n";
        let rendered = render_markdown(md);
        assert_eq!(rendered.outline.len(), 1);
        assert_eq!(rendered.outline[0].text, "With bold and code and em");
    }

    #[test]
    fn outline_decodes_html_entities() {
        let md = "## Foo & Bar < Baz > Qux \"quoted\"\n";
        let rendered = render_markdown(md);
        assert_eq!(rendered.outline.len(), 1);
        assert_eq!(rendered.outline[0].text, "Foo & Bar < Baz > Qux \"quoted\"");
    }

    #[test]
    fn outline_id_matches_anchor_link() {
        let md = "## Hello World\n";
        let rendered = render_markdown(md);
        assert_eq!(rendered.outline.len(), 1);
        let id = &rendered.outline[0].id;
        assert!(
            rendered.html.contains(&format!(r##"href="#{id}""##)),
            "id `{id}` should match an anchor href in the rendered HTML"
        );
    }
}
