//! UI components built with Leptos.
//!
//! - [`chrome`] - Shared site chrome primitives
//! - [`router`] - Application routing (main entry point)
//! - [`Shell`] - Main shell interface (terminal/explorer container)
//! - [`breadcrumb`] - Shared breadcrumb navigation component
//! - [`editor`] - Edit modal for text files
//! - [`explorer`] - File browser UI
//! - [`icons`] - Centralized icon definitions (change theme here)
//! - [`ledger_page`] - Ledger-style content index pages
//! - [`markdown`] - Shared Markdown rendering components
//! - [`reader`] - View/edit reader page (also handles `/new` compose)
//! - [`terminal`] - Terminal emulator interface

pub mod breadcrumb;
pub mod chrome;
pub mod editor;
pub mod explorer;
pub mod home;
pub mod icons;
pub mod ledger_page;
pub mod ledger_routes;
pub mod markdown;
pub mod mempool;
pub mod reader;
pub mod router;
pub mod shared;
pub mod terminal;
pub mod wallet;

pub use breadcrumb::Breadcrumb;
pub use home::HomePage;
pub use router::RouterView;
