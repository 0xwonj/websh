//! Terminal-related data types for output rendering.

use std::sync::atomic::{AtomicU64, Ordering};

/// Unique identifier for an `OutputLine`, used as a stable UI list key.
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
        date: Option<String>,
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
    pub fn long_entry(
        entry: &crate::domain::DirEntry,
        perms: &crate::domain::DisplayPermissions,
    ) -> Self {
        let style = if entry.is_dir {
            TextStyle::Directory
        } else if entry.name.starts_with('.') {
            TextStyle::Hidden
        } else {
            TextStyle::File
        };
        let meta = entry.meta.as_ref();
        Self::new(OutputLineData::ListEntry {
            name: entry.name.clone(),
            description: entry.title.clone(),
            style,
            encrypted: meta.map(|m| m.is_restricted()).unwrap_or(false),
            format: ListFormat::Long {
                permissions: perms.to_string(),
                size: meta.and_then(|m| m.size_bytes()),
                date: meta.and_then(|m| m.date()).map(str::to_string),
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
    fn file_listing_style_distinguishes_hidden_names() {
        for (name, expected) in [
            ("readme.md", TextStyle::File),
            (".gitignore", TextStyle::Hidden),
        ] {
            let entry = OutputLine::file_entry(name, "", false);
            assert!(
                matches!(entry.data, OutputLineData::ListEntry { style, .. } if style == expected)
            );
        }
    }

    #[test]
    fn repeated_output_gets_distinct_monotonic_ids() {
        let first = OutputLine::text("same text");
        let second = OutputLine::text("same text");
        assert_eq!(first.data, second.data);
        assert!(first.id.0 < second.id.0);
        assert_ne!(first, second);
    }
}
