//! Application signal containers.

use leptos::prelude::*;

use super::ring_buffer::RingBuffer;
use crate::config::{MAX_COMMAND_HISTORY, MAX_TERMINAL_HISTORY};
use websh_core::shell::OutputLine;

// The state container structs in this module derive `Clone` and `Copy`.
// This is intentional: every field is a Leptos reactive handle (`RwSignal`,
// `Memo`, `StoredValue`), which itself is a cheap `Copy` pointer into Leptos'
// reactive arena. Copying one of these containers therefore duplicates a
// handful of pointers, not the underlying state.

/// Terminal state managed with Leptos signals.
#[derive(Clone, Copy)]
pub struct TerminalState {
    /// Terminal output history (bounded by `MAX_TERMINAL_HISTORY`).
    pub history: RwSignal<RingBuffer<OutputLine>>,
    /// Command history for up/down navigation.
    pub command_history: RwSignal<Vec<String>>,
    /// Current position in command history (for navigation).
    pub history_index: RwSignal<Option<usize>>,
}

impl TerminalState {
    pub fn new() -> Self {
        Self {
            history: RwSignal::new(RingBuffer::new(MAX_TERMINAL_HISTORY)),
            command_history: RwSignal::new(Vec::new()),
            history_index: RwSignal::new(None),
        }
    }

    pub fn push_output(&self, line: OutputLine) {
        self.history.update(|h| h.push(line));
    }

    pub fn push_lines(&self, lines: Vec<OutputLine>) {
        if lines.is_empty() {
            return;
        }
        self.history.update(|h| {
            h.extend(lines);
            h.push(OutputLine::empty());
        });
    }

    pub fn clear_history(&self) {
        self.history.update(|h| h.clear());
    }

    pub fn add_to_command_history(&self, cmd: &str) {
        if !cmd.trim().is_empty() {
            self.command_history.update(|h| {
                if h.last().map(|s| s.as_str()) != Some(cmd) {
                    h.push(cmd.to_string());
                    if h.len() > MAX_COMMAND_HISTORY {
                        h.remove(0);
                    }
                }
            });
        }
        self.history_index.set(None);
    }

    pub fn navigate_history(&self, direction: i32) -> Option<String> {
        let current_index = self.history_index.get();
        let (new_index, result) = self.command_history.with(|history| {
            if history.is_empty() {
                return (None, None);
            }
            let new_index = match current_index {
                None if direction < 0 => Some(history.len() - 1),
                Some(i) if direction < 0 && i > 0 => Some(i - 1),
                Some(i) if direction > 0 && i < history.len() - 1 => Some(i + 1),
                Some(_) if direction > 0 => None,
                _ => current_index,
            };
            let result = new_index.map(|i| history[i].clone());
            (new_index, result)
        });
        self.history_index.set(new_index);
        result
    }
}

impl Default for TerminalState {
    fn default() -> Self {
        Self::new()
    }
}
