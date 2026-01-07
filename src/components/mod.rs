//! UI components built with Leptos.
//!
//! - [`Shell`] - Main shell interface (root component)
//! - [`explorer`] - File browser UI
//! - [`icons`] - Centralized icon definitions (change theme here)
//! - [`reader`] - Content reader for markdown, PDF, images
//! - [`status`] - Status bar showing session and location info
//! - [`terminal`] - Terminal emulator interface

pub mod explorer;
pub mod icons;
pub mod reader;
pub mod status;
pub mod terminal;

pub use terminal::Shell;
