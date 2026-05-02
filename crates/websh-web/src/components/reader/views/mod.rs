//! Per-intent reader view components. Each file owns one intent's body.

pub mod asset;
pub mod html;
pub mod markdown;
pub mod pdf;
pub mod plain;
pub mod redirect;

pub use asset::AssetReaderView;
pub use html::HtmlReaderView;
pub use markdown::{MarkdownEditorView, MarkdownReaderView};
pub use pdf::PdfReaderView;
pub use plain::PlainReaderView;
pub use redirect::RedirectingView;
