use crate::models::{OutputLine, OutputLineData, TextStyle};
use leptos::prelude::*;

stylance::import_crate_style!(css, "src/components/terminal/output.module.css");

/// Get CSS class for a TextStyle
fn style_class(style: TextStyle) -> &'static str {
    match style {
        TextStyle::Directory => css::textCyan,
        TextStyle::File => css::textFg,
        TextStyle::Hidden => css::textDim,
    }
}

#[component]
pub fn Output(line: OutputLine) -> impl IntoView {
    match line.data {
        OutputLineData::Command { prompt, input } => {
            view! {
                <div class=css::command>
                    <span class=format!("{} glow", css::textGreen)>{prompt}</span>
                    <span class=css::textDim>"$ "</span>
                    <span class=css::textFg>{input}</span>
                </div>
            }.into_any()
        }
        OutputLineData::Text(text) => {
            view! {
                <div class=format!("{} {}", css::line, css::textFg)>{text}</div>
            }.into_any()
        }
        OutputLineData::ListEntry { name, description, style } => {
            let is_dir = style == TextStyle::Directory;
            let name_class = if is_dir {
                format!("{} {}", style_class(style), css::fontBold)
            } else {
                style_class(style).to_string()
            };
            let suffix = if is_dir { "/" } else { "" };
            view! {
                <div class=css::listEntry>
                    <span class=name_class>{format!("{}{}", name, suffix)}</span>
                    <span class=css::textDim>{description}</span>
                </div>
            }.into_any()
        }
        OutputLineData::Error(text) => {
            view! {
                <div class=format!("{} {}", css::line, css::textRed)>{text}</div>
            }.into_any()
        }
        OutputLineData::Success(text) => {
            view! {
                <div class=format!("{} {}", css::line, css::textGreen)>{text}</div>
            }.into_any()
        }
        OutputLineData::Info(text) => {
            view! {
                <div class=format!("{} {}", css::line, css::textYellow)>{text}</div>
            }.into_any()
        }
        OutputLineData::Ascii(text) => {
            view! {
                <pre class=format!("{} glow", css::ascii)>{text}</pre>
            }.into_any()
        }
        OutputLineData::Empty => {
            view! {
                <div class=css::lineEmpty></div>
            }.into_any()
        }
    }
}
