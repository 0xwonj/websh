use super::*;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
fn sanitizer_keeps_safe_content_and_removes_active_content() {
    let cases: &[(&str, &str, &[&str])] = &[
        (
            r#"<img src="x" onerror="alert(1)"><script>alert(2)</script>"#,
            "<img",
            &["onerror", "<script"],
        ),
        (
            r#"<a href="jav&#x61;script:alert(1)">bad</a>"#,
            ">bad</a>",
            &["href", "javascript"],
        ),
        (
            r#"<img alt="x" src="data:image/svg+xml,boom">"#,
            r#"alt="x""#,
            &["src=", "data:"],
        ),
        (
            r#"<p onclick=alert(1) title=safe>text</p>"#,
            r#"<p title="safe">text</p>"#,
            &["onclick"],
        ),
        (
            r#"<p>safe</p><style>.x{}</style><script>alert(1)</script><iframe><p>bad</p></iframe>"#,
            "<p>safe</p>",
            &["<style", "<script", "<iframe"],
        ),
        (
            r#"<a href="../notes?q=1#top">notes</a>"#,
            r#"<a href="../notes?q=1#top" rel="noopener noreferrer">notes</a>"#,
            &[],
        ),
    ];
    for (input, kept, removed) in cases {
        let html = sanitize_html(input);
        assert!(html.contains(kept), "input={input:?}, output={html}");
        for value in *removed {
            assert!(!html.contains(value), "input={input:?}, output={html}");
        }
    }
}

#[wasm_bindgen_test]
fn block_and_inline_markdown_keep_links_with_their_own_wrapping() {
    let markdown = "Writing [tabula](/#/papers/tabula).";
    for (inline, rendered) in [
        (false, render_markdown(markdown)),
        (true, render_inline_markdown(markdown)),
    ] {
        assert!(
            rendered
                .html
                .contains(r#"<a href="/#/papers/tabula" rel="noopener noreferrer">tabula</a>"#)
        );
        assert_eq!(rendered.html.starts_with("<p>"), !inline);
    }
    assert!(
        !render_inline_markdown("![image](relative.png)")
            .html
            .contains("<img")
    );
}

#[wasm_bindgen_test]
fn markdown_does_not_render_raw_active_html() {
    let rendered = render_markdown(r#"<script>alert(1)</script><img src=x onerror=alert(2)>"#);
    assert!(!rendered.html.contains("<script"));
    assert!(!rendered.html.contains("onerror"));
}

#[wasm_bindgen_test]
fn math_placeholders_distinguish_inline_display_and_literal_dollars() {
    for (input, style, text) in [
        ("$E = mc^2$", "inline", "E = mc^2"),
        ("$$x^2$$", "display", "x^2"),
    ] {
        let rendered = render_markdown(input);
        assert!(rendered.has_math, "{input}");
        assert!(
            rendered
                .html
                .contains(&format!(r#"data-math-style="{style}""#)),
            "{}",
            rendered.html
        );
        assert!(rendered.html.contains(text));
    }
    let literal = render_markdown("Cost is \\$5.");
    assert!(!literal.has_math);
    assert!(literal.html.contains("Cost is $5."));
}

#[wasm_bindgen_test]
fn outline_preserves_order_text_and_matching_anchors_for_h2_h3() {
    let rendered = render_markdown(
        "# Title\n\n## With **bold**, `code`, *em* & \"quotes\"\n\n### 효과와 한계\n\n## Section B\n\n#### H4\n##### H5\n###### H6\n",
    );
    let items: Vec<_> = rendered
        .outline
        .iter()
        .map(|entry| (entry.level, entry.text.as_str()))
        .collect();
    assert_eq!(
        items,
        [
            (2, "With bold, code, em & \"quotes\""),
            (3, "효과와 한계"),
            (2, "Section B")
        ]
    );
    for entry in rendered.outline {
        assert!(
            rendered
                .html
                .contains(&format!(r##"href="#{}""##, entry.id)),
            "{}",
            entry.id
        );
    }
}

#[wasm_bindgen_test]
fn text_without_headings_has_no_outline() {
    assert!(render_markdown("Just a paragraph.\n").outline.is_empty());
}

#[wasm_bindgen_test]
fn render_markdown_preserves_tables_task_lists_and_footnotes() {
    let rendered = render_markdown(
        "| A | B |\n| - | - |\n| 1 | 2 |\n\n- [x] done\n\nfootnote[^a]\n\n[^a]: note",
    );
    assert!(rendered.html.contains("<table>"), "{}", rendered.html);
    assert!(rendered.html.contains("<td>1</td>"), "{}", rendered.html);
    assert!(
        rendered
            .html
            .contains(r#"<input checked="" disabled="" type="checkbox">"#)
            || rendered
                .html
                .contains(r#"<input type="checkbox" checked="" disabled="">"#),
        "{}",
        rendered.html
    );
    assert!(
        rendered.html.contains("data-footnote-ref")
            && rendered.html.contains("data-footnote-backref"),
        "{}",
        rendered.html
    );
}

#[wasm_bindgen_test]
fn rendered_raw_html_is_sanitized_before_metadata_extraction() {
    let rendered = rendered_from_html(sanitize_html(
        r#"<h2 id="safe">Safe <em>Title</em></h2><script>alert(1)</script><a href="javascript:alert(1)">x</a>"#,
    ));
    assert_eq!(rendered.outline.len(), 1);
    assert_eq!(rendered.outline[0].id, "safe");
    assert_eq!(rendered.outline[0].text, "Safe Title");
    assert!(!rendered.html.contains("<script"));
    assert!(!rendered.html.contains("javascript:"));
}

#[wasm_bindgen_test]
fn render_markdown_strips_frontmatter() {
    let rendered = render_markdown("---\ndate: 2026-04-26\ntags: [math]\n---\n\n# Body\n");
    assert!(rendered.html.contains(r##"<a href="#body""##));
    assert!(rendered.html.contains(r#"id="body""#));
    assert!(!rendered.html.contains("date:"));
    assert!(!rendered.html.contains("tags:"));
}

#[wasm_bindgen_test]
fn hydrate_math_without_math_does_not_inject_katex_assets() {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .expect("document should be available");
    if let Some(node) = document.get_element_by_id("websh-katex-css") {
        node.remove();
    }
    if let Some(node) = document.get_element_by_id("websh-katex-js") {
        node.remove();
    }
    reset_katex_loader();

    let root = document.create_element("div").expect("div");
    root.set_inner_html("<p>plain text</p>");
    hydrate_math(&root);

    assert!(document.get_element_by_id("websh-katex-css").is_none());
    assert!(document.get_element_by_id("websh-katex-js").is_none());
}
