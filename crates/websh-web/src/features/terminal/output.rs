use crate::shared::icons as ic;
use leptos::prelude::*;
use websh_core::shell::{ListFormat, OutputLine, OutputLineData, TextStyle};
use websh_core::support::format::{format_date_short, format_size};

stylance::import_crate_style!(css, "src/features/terminal/output.module.css");

/// Get CSS class for a TextStyle
fn style_class(style: TextStyle) -> &'static str {
    match style {
        TextStyle::Directory => css::textAccent,
        TextStyle::File => css::textFg,
        TextStyle::Hidden => css::textDim,
    }
}

#[component]
pub fn Output(line: OutputLine) -> impl IntoView {
    match line.data {
        OutputLineData::Command { prompt, input } => view! {
            <div class=css::command>
                <span class=format!("{} glow", css::textGreen)>{prompt}</span>
                <span class=css::textDim>"$ "</span>
                <span class=css::textFg>{input}</span>
            </div>
        }
        .into_any(),
        OutputLineData::Text(text) => view! {
            <div class=format!("{} {}", css::line, css::textDim)>{text}</div>
        }
        .into_any(),
        OutputLineData::ListEntry {
            name,
            description,
            style,
            encrypted,
            format,
        } => {
            let is_dir = style == TextStyle::Directory;
            let name_class = if is_dir {
                format!("{} {}", style_class(style), css::fontBold)
            } else {
                style_class(style).to_string()
            };
            let suffix = if is_dir { "/" } else { "" };
            let display_name = format!("{}{}", name, suffix);
            let lock_marker = encrypted.then(|| {
                view! {
                    <span class=css::lockIcon aria-label="encrypted">
                        <ic::SvgIcon icon=ic::LOCK />
                    </span>
                }
            });

            match format {
                ListFormat::Short => view! {
                    <div class=css::listEntry>
                        <span class=name_class>
                            {display_name}
                            {lock_marker}
                        </span>
                        <span class=css::textDim>{description}</span>
                    </div>
                }
                .into_any(),
                ListFormat::Long {
                    permissions,
                    size,
                    modified,
                } => view! {
                    <div class=css::longEntry>
                        <span class=css::textDim>{permissions}</span>
                        <span class=css::textDim>{format_size(size, true)}</span>
                        <span class=css::textDim>{format_date_short(modified)}</span>
                        <span class=name_class>
                            {display_name}
                            {lock_marker}
                        </span>
                    </div>
                }
                .into_any(),
            }
        }
        OutputLineData::Error(text) => view! {
            <div class=format!("{} {}", css::line, css::textRed)>{text}</div>
        }
        .into_any(),
        OutputLineData::Success(text) => view! {
            <div class=format!("{} {}", css::line, css::textGreen)>{text}</div>
        }
        .into_any(),
        OutputLineData::Info(text) => view! {
            <div class=format!("{} {}", css::line, css::textYellow)>{text}</div>
        }
        .into_any(),
        OutputLineData::Ascii(text) => view! {
            <pre class=format!("{} glow", css::ascii)>{text}</pre>
        }
        .into_any(),
        OutputLineData::Empty => view! {
            <div class=css::lineEmpty></div>
        }
        .into_any(),
    }
}
