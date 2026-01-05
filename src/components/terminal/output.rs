use crate::models::{ListFormat, OutputLine, OutputLineData, TextStyle};
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

/// Format file size for display (e.g., "1.2K", "3.4M"), right-aligned
fn format_size(size: Option<u64>) -> String {
    match size {
        None => "    -".to_string(),
        Some(bytes) => {
            if bytes >= 1_000_000 {
                format!("{:4.1}M", bytes as f64 / 1_000_000.0)
            } else if bytes >= 1_000 {
                format!("{:4.1}K", bytes as f64 / 1_000.0)
            } else {
                format!("{:4}B", bytes)
            }
        }
    }
}

/// Format Unix timestamp for display (e.g., "Jan  5 12:34")
fn format_date(timestamp: Option<u64>) -> String {
    match timestamp {
        None => "            ".to_string(),
        Some(ts) => {
            // Simple date formatting without external crates
            let months = [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ];
            // Approximate: days since epoch
            let days = ts / 86400;
            let month = ((days % 365) / 30) as usize % 12;
            let day = ((days % 365) % 30) + 1;
            let hour = (ts % 86400) / 3600;
            let min = (ts % 3600) / 60;
            format!("{} {:2} {:02}:{:02}", months[month], day, hour, min)
        }
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
            <div class=format!("{} {}", css::line, css::textFg)>{text}</div>
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
            let lock_icon = if encrypted { " 🔒" } else { "" };

            match format {
                ListFormat::Short => view! {
                    <div class=css::listEntry>
                        <span class=name_class>{format!("{}{}{}", name, suffix, lock_icon)}</span>
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
                        <span class=css::textDim>{format_size(size)}</span>
                        <span class=css::textDim>{format_date(modified)}</span>
                        <span class=name_class>{format!("{}{}{}", name, suffix, lock_icon)}</span>
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
