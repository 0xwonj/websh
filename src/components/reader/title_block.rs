//! `Ident` strip + `TitleBlock` (h1 + per-intent `MetaTable`).

use leptos::prelude::*;

use crate::components::shared::{MetaRow, MetaTable};

use super::css;
use super::intent::ReaderIntent;
use super::meta::ReaderMeta;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowSpec {
    Type {
        tag: String,
        hint: Option<&'static str>,
    },
    Size {
        value: String,
    },
    Date {
        value: String,
    },
    Tags {
        items: Vec<String>,
    },
    Caption {
        text: String,
    },
}

pub fn rows_for(intent: &ReaderIntent, meta: &ReaderMeta) -> Vec<RowSpec> {
    let mut rows = Vec::new();

    let type_tag = match intent {
        ReaderIntent::Markdown { .. } => Some("markdown".to_string()),
        ReaderIntent::Html { .. } => Some("html".to_string()),
        ReaderIntent::Plain { .. } => Some("text".to_string()),
        ReaderIntent::Asset { media_type, .. } => Some(media_type.clone()),
        ReaderIntent::Redirect { .. } => None,
    };

    if let Some(tag) = type_tag {
        rows.push(RowSpec::Type {
            tag,
            hint: meta.media_type_hint,
        });
    }

    let wants_size = !matches!(
        intent,
        ReaderIntent::Markdown { .. } | ReaderIntent::Html { .. } | ReaderIntent::Redirect { .. }
    );
    if wants_size && let Some(size) = meta.size_pretty.clone() {
        rows.push(RowSpec::Size { value: size });
    }

    if !matches!(intent, ReaderIntent::Redirect { .. })
        && let Some(date) = meta.display_date()
    {
        rows.push(RowSpec::Date { value: date });
    }

    let wants_tags = matches!(intent, ReaderIntent::Markdown { .. })
        || matches!(intent, ReaderIntent::Asset { media_type, .. } if media_type == "application/pdf");
    if wants_tags && !meta.tags.is_empty() {
        rows.push(RowSpec::Tags {
            items: meta.tags.clone(),
        });
    }

    let wants_caption = matches!(intent, ReaderIntent::Asset { media_type, .. } if media_type.starts_with("image/"));
    if wants_caption && !meta.description.is_empty() {
        rows.push(RowSpec::Caption {
            text: meta.description.clone(),
        });
    }

    rows
}

#[component]
pub fn Ident(meta: Memo<ReaderMeta>) -> impl IntoView {
    view! {
        {move || {
            let m = meta.get();
            let path = m.canonical_path.as_str().to_string();
            let date = m.display_date();
            // `canonical_path.as_str()` is always at least "/", so the path
            // span is always present; the right-hand `Date` span is rendered
            // only when a date is available.
            view! {
                <div class=css::ident>
                    <span class=css::identId><b>{path}</b></span>
                    {date.map(|value| view! {
                        <span class=css::identRev>{value}</span>
                    })}
                </div>
            }
        }}
    }
}

#[component]
pub fn TitleBlock(intent: Memo<ReaderIntent>, meta: Memo<ReaderMeta>) -> impl IntoView {
    view! {
        <div class=css::titleBlock>
            <h1 class=css::title>{move || meta.get().title.clone()}</h1>
            {move || {
                let i = intent.get();
                let m = meta.get();
                let rows = rows_for(&i, &m);
                if rows.is_empty() {
                    None
                } else {
                    Some(view! {
                        <MetaTable class=css::metaTable aria_label="file metadata">
                            {rows.into_iter().map(render_row).collect_view()}
                        </MetaTable>
                    })
                }
            }}
        </div>
    }
}

fn render_row(spec: RowSpec) -> AnyView {
    match spec {
        RowSpec::Type { tag, hint } => view! {
            <MetaRow
                label="Type"
                row_class=css::metaRow
                key_class=css::metaKey
                value_class=css::metaValue
            >
                <span class=css::metaTag>{tag}</span>
                {hint.map(|h| view! { <span class=css::metaDim>{h}</span> })}
            </MetaRow>
        }
        .into_any(),
        RowSpec::Size { value } => view! {
            <MetaRow
                label="Size"
                row_class=css::metaRow
                key_class=css::metaKey
                value_class=css::metaValue
            >
                {value}
            </MetaRow>
        }
        .into_any(),
        RowSpec::Date { value } => view! {
            <MetaRow
                label="Date"
                row_class=css::metaRow
                key_class=css::metaKey
                value_class=css::metaValue
            >
                {value}
            </MetaRow>
        }
        .into_any(),
        RowSpec::Tags { items } => view! {
            <MetaRow
                label="Tags"
                row_class=css::metaRow
                key_class=css::metaKey
                value_class=css::metaValue
            >
                {items.into_iter().map(|tag| view! {
                    <span class=css::metaTag>{tag}</span>
                }).collect_view()}
            </MetaRow>
        }
        .into_any(),
        RowSpec::Caption { text } => view! {
            <MetaRow
                label="Caption"
                row_class=css::metaRow
                key_class=css::metaKey
                value_class=css::metaValue
            >
                {text}
            </MetaRow>
        }
        .into_any(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::VirtualPath;

    fn vp(path: &str) -> VirtualPath {
        VirtualPath::from_absolute(path).expect("test path")
    }

    fn meta_with(date: Option<&str>, modified_iso: Option<&str>) -> ReaderMeta {
        ReaderMeta {
            title: "x".to_string(),
            canonical_path: vp("/x.md"),
            modified_iso: modified_iso.map(String::from),
            date: date.map(String::from),
            size_pretty: None,
            tags: vec![],
            description: String::new(),
            media_type_hint: Some("UTF-8 · CommonMark"),
        }
    }

    #[test]
    fn date_row_prefers_author_declared() {
        let intent = ReaderIntent::Markdown {
            node_path: vp("/x.md"),
        };
        let m = meta_with(Some("2026-04-22"), Some("2026-04-30"));
        let rows = rows_for(&intent, &m);
        let date = rows.iter().find_map(|r| match r {
            RowSpec::Date { value } => Some(value.clone()),
            _ => None,
        });
        assert_eq!(date.as_deref(), Some("2026-04-22"));
    }

    #[test]
    fn date_row_falls_back_to_modified() {
        let intent = ReaderIntent::Markdown {
            node_path: vp("/x.md"),
        };
        let m = meta_with(None, Some("2026-04-30"));
        let rows = rows_for(&intent, &m);
        let date = rows.iter().find_map(|r| match r {
            RowSpec::Date { value } => Some(value.clone()),
            _ => None,
        });
        assert_eq!(date.as_deref(), Some("2026-04-30"));
    }

    #[test]
    fn date_row_omitted_when_both_absent() {
        let intent = ReaderIntent::Markdown {
            node_path: vp("/x.md"),
        };
        let m = meta_with(None, None);
        let rows = rows_for(&intent, &m);
        assert!(
            rows.iter().all(|r| !matches!(r, RowSpec::Date { .. })),
            "no Date row expected, got {rows:?}"
        );
    }

    #[test]
    fn plain_emits_size_row_when_present() {
        let intent = ReaderIntent::Plain {
            node_path: vp("/x.txt"),
        };
        let mut m = meta_with(None, None);
        m.size_pretty = Some("2 KB".to_string());
        let rows = rows_for(&intent, &m);
        assert!(
            rows.iter().any(|r| matches!(r, RowSpec::Size { .. })),
            "expected Size row, got {rows:?}"
        );
    }

    #[test]
    fn redirect_emits_no_rows() {
        let intent = ReaderIntent::Redirect {
            node_path: vp("/x.link"),
        };
        let m = meta_with(Some("2026-04-22"), None);
        let rows = rows_for(&intent, &m);
        assert!(rows.is_empty(), "redirect rows should be empty: {rows:?}");
    }

    #[test]
    fn image_caption_appears_only_when_description_set() {
        let intent = ReaderIntent::Asset {
            node_path: vp("/cover.png"),
            media_type: "image/png".to_string(),
        };
        let mut m = meta_with(None, None);
        m.description = "Sunrise.".to_string();
        let rows = rows_for(&intent, &m);
        assert!(
            rows.iter().any(|r| matches!(r, RowSpec::Caption { .. })),
            "expected Caption row, got {rows:?}"
        );

        m.description = String::new();
        let rows2 = rows_for(&intent, &m);
        assert!(
            rows2.iter().all(|r| !matches!(r, RowSpec::Caption { .. })),
            "Caption should be omitted, got {rows2:?}"
        );
    }
}
