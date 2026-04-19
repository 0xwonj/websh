//! Markdown editor component with formatting toolbar.

use leptos::{ev, prelude::*};
use leptos_icons::Icon;
use wasm_bindgen::prelude::*;

use crate::components::icons as ic;

stylance::import_crate_style!(css, "src/components/reader/editor.module.css");

/// Image upload data passed to the callback.
#[derive(Clone, Debug)]
pub struct ImageUpload {
    /// Original filename
    pub filename: String,
    /// MIME type (e.g., "image/png")
    pub mime_type: String,
    /// Base64-encoded content
    pub content_base64: String,
}

/// Apply syntax highlighting to markdown content.
/// Returns HTML with span elements for different syntax types.
fn highlight_markdown(content: &str) -> String {
    let mut result = String::new();

    for line in content.lines() {
        if !result.is_empty() {
            result.push('\n');
        }

        // Check for headers
        if line.starts_with("######") {
            result.push_str(&format!(r#"<span class="h6">{}</span>"#, escape_html(line)));
        } else if line.starts_with("#####") {
            result.push_str(&format!(r#"<span class="h5">{}</span>"#, escape_html(line)));
        } else if line.starts_with("####") {
            result.push_str(&format!(r#"<span class="h4">{}</span>"#, escape_html(line)));
        } else if line.starts_with("###") {
            result.push_str(&format!(r#"<span class="h3">{}</span>"#, escape_html(line)));
        } else if line.starts_with("##") {
            result.push_str(&format!(r#"<span class="h2">{}</span>"#, escape_html(line)));
        } else if line.starts_with('#') {
            result.push_str(&format!(r#"<span class="h1">{}</span>"#, escape_html(line)));
        } else if line.starts_with('>') {
            result.push_str(&format!(r#"<span class="quote">{}</span>"#, escape_html(line)));
        } else if line.starts_with("```") {
            result.push_str(&format!(r#"<span class="code-fence">{}</span>"#, escape_html(line)));
        } else if line.starts_with("---") || line.starts_with("***") || line.starts_with("___") {
            result.push_str(&format!(r#"<span class="hr">{}</span>"#, escape_html(line)));
        } else {
            result.push_str(&escape_html(line));
        }
    }

    // Handle trailing newline
    if content.ends_with('\n') {
        result.push('\n');
    }

    result
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Markdown editor with line numbers and optional formatting toolbar.
#[component]
pub fn Editor(
    /// Current content signal
    content: RwSignal<String>,
    /// Whether to show the formatting toolbar
    #[prop(default = true)]
    show_toolbar: bool,
    /// Callback when an image is uploaded. Returns the path to insert into markdown.
    #[prop(optional)]
    on_image_upload: Option<Callback<ImageUpload, String>>,
    /// Callback when save is requested (Ctrl+S)
    #[prop(optional)]
    on_save: Option<Callback<()>>,
) -> impl IntoView {
    // Reference to textarea for formatting insertions
    let textarea_ref = NodeRef::<leptos::html::Textarea>::new();
    // Reference to hidden file input
    let file_input_ref = NodeRef::<leptos::html::Input>::new();

    // Track initial content to set only once on mount
    let initial_content = content.get_untracked();
    let (initialized, set_initialized) = signal(false);

    // Set initial value only once when textarea is mounted
    Effect::new(move || {
        if !initialized.get() {
            if let Some(textarea) = textarea_ref.get() {
                textarea.set_value(&initial_content);
                set_initialized.set(true);
            }
        }
    });

    // Format button handlers
    let insert_format = move |prefix: &'static str, suffix: &'static str| {
        move |_: ev::MouseEvent| {
            if let Some(textarea) = textarea_ref.get() {
                let start = textarea.selection_start().ok().flatten().unwrap_or(0) as usize;
                let end = textarea.selection_end().ok().flatten().unwrap_or(0) as usize;
                let current = textarea.value();

                let (before, rest) = current.split_at(start.min(current.len()));
                let (selected, after) = rest.split_at((end - start).min(rest.len()));

                let new_content = format!("{}{}{}{}{}", before, prefix, selected, suffix, after);

                // Update both textarea and signal
                textarea.set_value(&new_content);
                content.set(new_content);

                // Restore cursor position after the inserted prefix
                let new_pos = (start + prefix.len() + selected.len() + suffix.len()) as u32;
                let _ = textarea.set_selection_start(Some(new_pos));
                let _ = textarea.set_selection_end(Some(new_pos));
                let _ = textarea.focus();
            }
        }
    };

    let insert_line_prefix = move |prefix: &'static str| {
        move |_: ev::MouseEvent| {
            if let Some(textarea) = textarea_ref.get() {
                let pos = textarea.selection_start().ok().flatten().unwrap_or(0) as usize;
                let current = textarea.value();

                // Find the start of the current line
                let line_start = current[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);

                let new_content = format!(
                    "{}{}{}",
                    &current[..line_start],
                    prefix,
                    &current[line_start..]
                );

                // Update both textarea and signal
                textarea.set_value(&new_content);
                content.set(new_content);

                // Adjust cursor position
                let new_pos = (pos + prefix.len()) as u32;
                let _ = textarea.set_selection_start(Some(new_pos));
                let _ = textarea.set_selection_end(Some(new_pos));
                let _ = textarea.focus();
            }
        }
    };

    // Image button click handler - triggers file input
    let handle_image_click = move |_: ev::MouseEvent| {
        if let Some(input) = file_input_ref.get() {
            input.click();
        }
    };

    // File input change handler - reads file and calls callback
    let handle_file_change = move |ev: ev::Event| {
        use web_sys::{FileReader, HtmlInputElement};

        let target: HtmlInputElement = event_target(&ev);
        let files = match target.files() {
            Some(f) => f,
            None => return,
        };

        let file = match files.get(0) {
            Some(f) => f,
            None => return,
        };

        let filename = file.name();
        let mime_type = file.type_();

        // Read file as base64
        let reader = match FileReader::new() {
            Ok(r) => r,
            Err(_) => return,
        };

        let reader_clone = reader.clone();
        let on_image_upload = on_image_upload;

        let onload = Closure::wrap(Box::new(move |_: web_sys::Event| {
            let result = reader_clone.result().ok();
            let data_url = result.and_then(|r| r.as_string());

            if let Some(data_url) = data_url {
                // Extract base64 part from data URL
                // Format: "data:image/png;base64,XXXX"
                if let Some(base64_start) = data_url.find(",") {
                    let content_base64 = data_url[base64_start + 1..].to_string();

                    let upload = ImageUpload {
                        filename: filename.clone(),
                        mime_type: mime_type.clone(),
                        content_base64,
                    };

                    // Call callback to get the path
                    let path = if let Some(cb) = on_image_upload {
                        cb.run(upload)
                    } else {
                        // Default: just insert placeholder
                        format!("/.assets/{}", filename)
                    };

                    // Insert markdown image syntax
                    if let Some(textarea) = textarea_ref.get() {
                        let pos = textarea.selection_start().ok().flatten().unwrap_or(0) as usize;
                        let current = textarea.value();

                        let image_md = format!("![{}]({})", filename, path);
                        let new_content = format!(
                            "{}{}{}",
                            &current[..pos.min(current.len())],
                            image_md,
                            &current[pos.min(current.len())..]
                        );

                        textarea.set_value(&new_content);
                        content.set(new_content);

                        let new_pos = (pos + image_md.len()) as u32;
                        let _ = textarea.set_selection_start(Some(new_pos));
                        let _ = textarea.set_selection_end(Some(new_pos));
                        let _ = textarea.focus();
                    }
                }
            }
        }) as Box<dyn FnMut(_)>);

        reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget(); // Prevent closure from being dropped

        let _ = reader.read_as_data_url(&file);

        // Clear the input so the same file can be selected again
        target.set_value("");
    };

    // Keyboard handler for textarea (Ctrl+S to save)
    let handle_keydown = move |ev: ev::KeyboardEvent| {
        let key = ev.key();
        // Ctrl+S or Cmd+S to save
        if (ev.ctrl_key() || ev.meta_key()) && key == "s" {
            ev.prevent_default();
            web_sys::console::log_1(&"Editor: Ctrl+S pressed".into());
            if let Some(cb) = on_save {
                web_sys::console::log_1(&"Editor: calling on_save callback".into());
                cb.run(());
            }
        }
    };

    view! {
        <div class=css::editor>
            // Formatting toolbar
            <Show when=move || show_toolbar>
                <div class=css::toolbar>
                    <button
                        class=css::button
                        title="Bold (Ctrl+B)"
                        on:click=insert_format("**", "**")
                    >
                        <span class=css::iconBold>"B"</span>
                    </button>
                    <button
                        class=css::button
                        title="Italic (Ctrl+I)"
                        on:click=insert_format("*", "*")
                    >
                        <span class=css::iconItalic>"I"</span>
                    </button>
                    <button
                        class=css::button
                        title="Strikethrough"
                        on:click=insert_format("~~", "~~")
                    >
                        <span class=css::iconStrike>"S"</span>
                    </button>
                    <span class=css::divider />
                    <button
                        class=css::button
                        title="Heading"
                        on:click=insert_line_prefix("## ")
                    >
                        <span class=css::icon>"H"</span>
                    </button>
                    <span class=css::divider />
                    <button
                        class=css::button
                        title="Code"
                        on:click=insert_format("`", "`")
                    >
                        <Icon icon=ic::FILE_TEXT />
                    </button>
                    <button
                        class=css::button
                        title="Quote"
                        on:click=insert_line_prefix("> ")
                    >
                        <span class=css::icon>"\""</span>
                    </button>
                    <span class=css::divider />
                    <button
                        class=css::button
                        title="Unordered List"
                        on:click=insert_line_prefix("- ")
                    >
                        <Icon icon=ic::LIST />
                    </button>
                    <button
                        class=css::button
                        title="Ordered List"
                        on:click=insert_line_prefix("1. ")
                    >
                        <span class=css::icon>"1."</span>
                    </button>
                    <button
                        class=css::button
                        title="Checkbox"
                        on:click=insert_line_prefix("- [ ] ")
                    >
                        <span class=css::icon>"[ ]"</span>
                    </button>
                    <span class=css::divider />
                    <button
                        class=css::button
                        title="Link"
                        on:click=insert_format("[", "](url)")
                    >
                        <Icon icon=ic::FILE_LINK />
                    </button>
                    <button
                        class=css::button
                        title="Upload Image"
                        on:click=handle_image_click
                    >
                        <Icon icon=ic::FILE_IMAGE />
                    </button>
                    // Hidden file input for image upload
                    <input
                        node_ref=file_input_ref
                        type="file"
                        accept="image/*"
                        style="display: none;"
                        on:change=handle_file_change
                    />
                    <button
                        class=css::button
                        title="Table"
                        on:click=insert_format("| Column 1 | Column 2 |\n|----------|----------|\n| ", " |  |")
                    >
                        <Icon icon=ic::GRID />
                    </button>
                    <span class=css::divider />
                    <button
                        class=css::button
                        title="Horizontal Rule"
                        on:click=insert_line_prefix("\n---\n")
                    >
                        <span class=css::icon>"—"</span>
                    </button>
                </div>
            </Show>

            // Editor area with line numbers and syntax highlighting
            <div class=css::editorArea>
                <div class=css::lineNumbers>
                    {move || {
                        let text = content.get();
                        // Count lines properly: lines() doesn't count trailing newline
                        let mut lines = text.lines().count();
                        if text.ends_with('\n') || text.is_empty() {
                            lines += 1;
                        }
                        (1..=lines).map(|n| view! {
                            <div class=css::lineNumber>{n}</div>
                        }).collect_view()
                    }}
                </div>
                <div class=css::editorContent>
                    // Syntax highlighted backdrop
                    <pre
                        class=css::highlightBackdrop
                        aria-hidden="true"
                        inner_html=move || highlight_markdown(&content.get())
                    />
                    // Transparent textarea on top
                    // Note: We don't use prop:value to preserve browser's native undo/redo behavior
                    <textarea
                        node_ref=textarea_ref
                        class=css::textarea
                        on:input=move |ev| content.set(event_target_value(&ev))
                        on:keydown=handle_keydown
                        spellcheck="false"
                        placeholder="Start writing..."
                    />
                </div>
            </div>
        </div>
    }
}
