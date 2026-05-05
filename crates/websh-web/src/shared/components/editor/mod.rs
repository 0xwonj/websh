//! Editor UI components.
//!
//! Currently hosts a minimal `EditModal`. The app layer drives visibility,
//! reads, and save/cancel callbacks.

pub mod modal;

pub use modal::EditModal;
