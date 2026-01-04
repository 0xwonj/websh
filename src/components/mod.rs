//! UI components built with Leptos.
//!
//! - [`Shell`] - Main terminal interface (root component)
//! - [`reader`] - Content reader for markdown, PDF, images
//! - [`status`] - Status bar showing session and location info

pub mod reader;
pub mod status;
pub mod terminal;

pub use terminal::Shell;
