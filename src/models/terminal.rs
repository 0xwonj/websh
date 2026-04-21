//! Terminal-related data types for output rendering.

use std::sync::atomic::{AtomicU64, Ordering};

/// Unique identifier for an `OutputLine`, used as a stable key in Leptos `For` lists.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct OutputLineId(pub u64);

/// Text styling for file listings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStyle {
    /// Directory entries (cyan, bold)
    Directory,
    /// Regular file entries
    File,
    /// Hidden files (dimmed)
    Hidden,
}

/// Format for file listing entries.
#[derive(Clone, Debug, PartialEq)]
pub enum ListFormat {
    /// Short format: name and description only
    Short,
    /// Long format: permissions, size, date, name
    Long {
        permissions: String,
        size: Option<u64>,
        modified: Option<u64>,
    },
}

/// Represents a single line of output in the terminal with a unique ID
#[derive(Clone, Debug, PartialEq)]
pub struct OutputLine {
    /// Unique ID for efficient keying in For loops
    pub id: OutputLineId,
    /// The actual output data
    pub data: OutputLineData,
}

/// The actual content of an output line
#[derive(Clone, Debug, PartialEq)]
pub enum OutputLineData {
    /// Command with prompt and user input
    Command { prompt: String, input: String },
    /// Plain text output
    Text(String),
    /// Error message (red)
    Error(String),
    /// Success message (green)
    Success(String),
    /// Info message (yellow)
    Info(String),
    /// ASCII art (with glow effect)
    Ascii(String),
    /// Empty line
    Empty,
    /// File listing entry (ls, ls -l)
    ListEntry {
        name: String,
        description: String,
        style: TextStyle,
        encrypted: bool,
        format: ListFormat,
    },
}

// Global counter for generating unique IDs
static OUTPUT_LINE_COUNTER: AtomicU64 = AtomicU64::new(0);

impl OutputLine {
    /// Create a new OutputLine with a unique ID
    fn new(data: OutputLineData) -> Self {
        Self {
            id: OutputLineId(OUTPUT_LINE_COUNTER.fetch_add(1, Ordering::Relaxed)),
            data,
        }
    }
}

impl OutputLine {
    pub fn text(s: impl Into<String>) -> Self {
        Self::new(OutputLineData::Text(s.into()))
    }

    pub fn error(s: impl Into<String>) -> Self {
        Self::new(OutputLineData::Error(s.into()))
    }

    pub fn success(s: impl Into<String>) -> Self {
        Self::new(OutputLineData::Success(s.into()))
    }

    pub fn info(s: impl Into<String>) -> Self {
        Self::new(OutputLineData::Info(s.into()))
    }

    pub fn ascii(s: impl Into<String>) -> Self {
        Self::new(OutputLineData::Ascii(s.into()))
    }

    pub fn command(prompt: impl Into<String>, input: impl Into<String>) -> Self {
        Self::new(OutputLineData::Command {
            prompt: prompt.into(),
            input: input.into(),
        })
    }

    /// Create a directory listing entry (short format)
    pub fn dir_entry(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(OutputLineData::ListEntry {
            name: name.into(),
            description: description.into(),
            style: TextStyle::Directory,
            encrypted: false,
            format: ListFormat::Short,
        })
    }

    /// Create a file listing entry (short format)
    pub fn file_entry(
        name: impl Into<String>,
        description: impl Into<String>,
        encrypted: bool,
    ) -> Self {
        let name = name.into();
        let style = if name.starts_with('.') {
            TextStyle::Hidden
        } else {
            TextStyle::File
        };
        Self::new(OutputLineData::ListEntry {
            name,
            description: description.into(),
            style,
            encrypted,
            format: ListFormat::Short,
        })
    }

    /// Create a long listing entry (ls -l)
    pub fn long_entry(entry: &crate::core::DirEntry, perms: &super::DisplayPermissions) -> Self {
        let style = if entry.is_dir {
            TextStyle::Directory
        } else if entry.name.starts_with('.') {
            TextStyle::Hidden
        } else {
            TextStyle::File
        };
        let file_meta = entry.file_meta.as_ref();
        Self::new(OutputLineData::ListEntry {
            name: entry.name.clone(),
            description: entry.title.clone(),
            style,
            encrypted: file_meta.map(|m| m.is_restricted()).unwrap_or(false),
            format: ListFormat::Long {
                permissions: perms.to_string(),
                size: file_meta.and_then(|m| m.size),
                modified: file_meta.and_then(|m| m.modified),
            },
        })
    }

    /// Create an empty line
    pub fn empty() -> Self {
        Self::new(OutputLineData::Empty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_line_constructors() {
        assert_eq!(
            OutputLine::text("hello").data,
            OutputLineData::Text("hello".to_string())
        );
        assert_eq!(
            OutputLine::error("error").data,
            OutputLineData::Error("error".to_string())
        );
        assert_eq!(
            OutputLine::success("ok").data,
            OutputLineData::Success("ok".to_string())
        );
        assert_eq!(
            OutputLine::info("info").data,
            OutputLineData::Info("info".to_string())
        );
        assert_eq!(
            OutputLine::ascii("art").data,
            OutputLineData::Ascii("art".to_string())
        );
    }

    #[test]
    fn test_command_line() {
        let cmd = OutputLine::command("user@host", "ls -la");
        match cmd.data {
            OutputLineData::Command { prompt, input } => {
                assert_eq!(prompt, "user@host");
                assert_eq!(input, "ls -la");
            }
            _ => panic!("Expected Command variant"),
        }
    }

    #[test]
    fn test_dir_entry() {
        let entry = OutputLine::dir_entry("docs", "Documentation");
        match entry.data {
            OutputLineData::ListEntry {
                name,
                description,
                style,
                encrypted,
                format,
            } => {
                assert_eq!(name, "docs");
                assert_eq!(description, "Documentation");
                assert_eq!(style, TextStyle::Directory);
                assert!(!encrypted);
                assert_eq!(format, ListFormat::Short);
            }
            _ => panic!("Expected ListEntry variant"),
        }
    }

    #[test]
    fn test_file_entry_normal() {
        let entry = OutputLine::file_entry("readme.md", "Readme file", false);
        match entry.data {
            OutputLineData::ListEntry { name, style, .. } => {
                assert_eq!(name, "readme.md");
                assert_eq!(style, TextStyle::File);
            }
            _ => panic!("Expected ListEntry variant"),
        }
    }

    #[test]
    fn test_file_entry_hidden() {
        let entry = OutputLine::file_entry(".gitignore", "Git ignore", false);
        match entry.data {
            OutputLineData::ListEntry { name, style, .. } => {
                assert_eq!(name, ".gitignore");
                assert_eq!(style, TextStyle::Hidden);
            }
            _ => panic!("Expected ListEntry variant"),
        }
    }

    #[test]
    fn test_unique_ids() {
        let line1 = OutputLine::text("first");
        let line2 = OutputLine::text("second");
        let line3 = OutputLine::text("first"); // Same content as line1

        // IDs should all be different
        assert_ne!(line1.id, line2.id);
        assert_ne!(line1.id, line3.id);
        assert_ne!(line2.id, line3.id);

        // But content equality works
        assert_eq!(line1.data, line3.data);
    }

    #[test]
    fn test_output_line_ids_are_unique_and_monotonic() {
        let a = OutputLine::text("a");
        let b = OutputLine::text("b");
        assert_ne!(a.id, b.id);
        // Newtype: compare via .0
        assert!(a.id.0 < b.id.0);
    }

    #[test]
    fn test_output_line_id_is_copy() {
        let a = OutputLine::text("a");
        let _copy = a.id; // Copy trait
        let _copy2 = a.id; // can copy twice
    }

    #[test]
    fn test_output_line_structural_eq() {
        let a = OutputLine::text("hello");
        let b = OutputLine::text("hello");
        // Different ids → structural PartialEq says not equal.
        assert_ne!(a, b);
        // But .data equality still works.
        assert_eq!(a.data, b.data);
    }
}
