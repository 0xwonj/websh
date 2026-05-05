//! Public filesystem facade.
//!
//! Callers should import filesystem APIs from this module instead of the
//! internal `engine` tree. The implementation remains internal so the file
//! layout can evolve without changing downstream imports.

pub use crate::engine::filesystem::*;
