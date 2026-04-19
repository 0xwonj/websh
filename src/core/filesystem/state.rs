//! Reactive filesystem state wrapper.
//!
//! Provides [`FsState`] which combines base VirtualFs and pending changes
//! into a unified reactive state with automatic MergedFs computation.

use leptos::prelude::*;

use super::{MergedFs, VirtualFs};
use crate::core::storage::{PendingChanges, StagedChanges};

/// Filesystem state wrapper with automatic MergedFs computation.
///
/// Encapsulates the base VirtualFs, pending changes, and staged changes.
/// Use `get()` to access the merged view (base + pending).
#[derive(Clone, Copy)]
pub struct FsState {
    /// Base filesystem loaded from manifest.
    base: RwSignal<VirtualFs>,
    /// Pending changes for admin mode.
    pending: RwSignal<PendingChanges>,
    /// Staged changes for commit.
    staged: RwSignal<StagedChanges>,
}

impl FsState {
    /// Creates a new FsState with the given pending changes.
    pub fn new(pending: PendingChanges) -> Self {
        let base = RwSignal::new(VirtualFs::empty());
        let pending = RwSignal::new(pending);
        let staged = RwSignal::new(StagedChanges::new());

        Self {
            base,
            pending,
            staged,
        }
    }

    /// Get the merged filesystem view (base + pending).
    pub fn get(&self) -> MergedFs {
        MergedFs::new(self.base.get(), self.pending.get())
    }

    /// Get the base VirtualFs.
    pub fn base(&self) -> VirtualFs {
        self.base.get()
    }

    /// Set the base VirtualFs (e.g., after loading manifest).
    pub fn set_base(&self, fs: VirtualFs) {
        self.base.set(fs);
    }

    /// Get the pending changes signal for direct access.
    pub fn pending(&self) -> RwSignal<PendingChanges> {
        self.pending
    }

    /// Get the staged changes signal for direct access.
    pub fn staged(&self) -> RwSignal<StagedChanges> {
        self.staged
    }

    /// Check if there are pending changes.
    pub fn has_pending(&self) -> bool {
        self.pending.with(|p| !p.is_empty())
    }

    /// Check if there are staged changes.
    pub fn has_staged(&self) -> bool {
        self.staged.with(|s| !s.is_empty())
    }

    /// Clear all pending changes.
    pub fn clear_pending(&self) {
        self.pending.update(|p| p.clear());
    }

    /// Clear all staged changes.
    pub fn clear_staged(&self) {
        self.staged.update(|s| s.clear());
    }

    /// Access merged filesystem with a closure (for reactive contexts).
    pub fn with<R>(&self, f: impl FnOnce(&MergedFs) -> R) -> R {
        let merged = MergedFs::new(self.base.get(), self.pending.get());
        f(&merged)
    }

    /// Access merged filesystem with a closure (untracked, for non-reactive contexts).
    pub fn with_untracked<R>(&self, f: impl FnOnce(&MergedFs) -> R) -> R {
        let merged = MergedFs::new(self.base.get_untracked(), self.pending.get_untracked());
        f(&merged)
    }
}

impl Default for FsState {
    fn default() -> Self {
        Self::new(PendingChanges::default())
    }
}
