//! Editor UI components.
//!
//! Currently hosts a minimal `EditModal` driven by `AppContext.editor_open`.
//! Opens when `edit <path>` dispatches `SideEffect::OpenEditor`; Save routes
//! through `dispatch_side_effect` as `SideEffect::ApplyChange` so all state
//! flows through the same pipe.

pub mod modal;

pub use modal::EditModal;
