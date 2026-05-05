//! Public runtime facade.
//!
//! Runtime construction, overlays, and commit planning are exposed here as a
//! stable bounded-context API. Internal module paths under `engine/runtime`
//! are not part of the public contract.

pub use crate::engine::runtime::*;
