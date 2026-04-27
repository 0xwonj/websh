//! HTML and Markdown rendering utilities.
//!
//! Provides safe HTML rendering boundaries with XSS protection.

use std::collections::{HashMap, HashSet};

use comrak::{Options, markdown_to_html as comrak_markdown_to_html};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RenderedMarkdown {
    pub html: String,
    pub has_math: bool,
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
    RenderedMarkdown { html, has_math }
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
}
