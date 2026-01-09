//! Preview components for file/directory content display.
//!
//! Provides both desktop (side panel) and mobile (bottom sheet) preview UIs.
//! The core logic is shared through [`use_preview`] hook.

mod content;
mod hook;
mod panel;
mod sheet;

pub use content::{OpenButton, PreviewBody, PreviewStyles};
pub use hook::{DirMeta, FileMeta, PreviewContent, PreviewData, use_preview};
pub use panel::PreviewPanel;
pub use sheet::BottomSheet;
