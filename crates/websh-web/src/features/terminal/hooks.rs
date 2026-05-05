//! Custom hooks for terminal components.
//!
//! Provides reusable stateful logic for terminal input handling.

use leptos::prelude::*;

/// State and operations for Tab-based autocompletion cycling.
///
/// When multiple matches exist for a Tab completion, this hook manages
/// cycling through them with repeated Tab presses.
#[derive(Clone, Copy)]
pub struct TabCycleState {
    /// All matching completions available for cycling.
    pub matches: RwSignal<Vec<String>>,
    /// Current index in the matches list.
    pub index: RwSignal<usize>,
    /// The common prefix or base text for completion.
    pub base: RwSignal<String>,
}

impl TabCycleState {
    /// Create a new Tab cycle state with empty values.
    pub fn new() -> Self {
        Self {
            matches: RwSignal::new(vec![]),
            index: RwSignal::new(0),
            base: RwSignal::new(String::new()),
        }
    }

    /// Check if currently in Tab cycling mode (has matches).
    pub fn is_active(&self) -> bool {
        self.matches.with(|m| !m.is_empty())
    }

    /// Clear all Tab cycling state.
    pub fn clear(&self) {
        self.matches.set(vec![]);
        self.index.set(0);
        self.base.set(String::new());
    }

    /// Advance to the next match in the cycle, returning the new index.
    pub fn advance(&self) -> usize {
        self.matches.with(|matches| {
            if matches.is_empty() {
                return 0;
            }
            let new_idx = (self.index.get() + 1) % matches.len();
            self.index.set(new_idx);
            new_idx
        })
    }

    /// Set up the cycle with new matches.
    pub fn start(&self, base: String, matches: Vec<String>) {
        self.base.set(base);
        self.matches.set(matches);
        self.index.set(0);
    }

    /// Get the currently selected match, if any.
    pub fn current_match(&self) -> Option<String> {
        self.matches.with(|matches| {
            let idx = self.index.get();
            matches.get(idx).cloned()
        })
    }

    /// Build the completed input value from base and current selection.
    ///
    /// Handles both command completion and path completion cases.
    pub fn build_completion(&self) -> Option<String> {
        self.base.with(|base| {
            let selected = self.current_match()?;

            let completed = if base.contains(' ') {
                // Path completion: base is "cmd prefix", selected is "name/"
                let parts: Vec<&str> = base.rsplitn(2, '/').collect();
                if parts.len() == 2 {
                    // Has directory part
                    format!("{}/{}", parts[1], selected.trim_end_matches('/'))
                } else {
                    // No directory, just command + name
                    let cmd_parts: Vec<&str> = base.splitn(2, ' ').collect();
                    if cmd_parts.len() == 2 {
                        format!("{} {}", cmd_parts[0], selected.trim_end_matches('/'))
                    } else {
                        base.clone()
                    }
                }
            } else {
                // Command completion
                selected
            };

            Some(completed)
        })
    }
}

impl Default for TabCycleState {
    fn default() -> Self {
        Self::new()
    }
}

/// State for ghost text hints shown while typing.
#[derive(Clone, Copy)]
pub struct HintState {
    /// Current hint text to display after user input.
    pub hint: RwSignal<Option<String>>,
}

impl HintState {
    /// Create a new hint state.
    pub fn new() -> Self {
        Self {
            hint: RwSignal::new(None),
        }
    }

    /// Get the current hint.
    pub fn get(&self) -> Option<String> {
        self.hint.get()
    }

    /// Set a new hint.
    pub fn set(&self, value: Option<String>) {
        self.hint.set(value);
    }

    /// Clear the hint.
    pub fn clear(&self) {
        self.hint.set(None);
    }
}

impl Default for HintState {
    fn default() -> Self {
        Self::new()
    }
}
