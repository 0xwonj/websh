use std::path::Path;

use clap::Args;

use crate::CliResult;
use crate::workflows::mempool::add::{self, AddOptions};

#[derive(Args)]
pub(super) struct AddArgs {
    /// Category segment (e.g., `writing`, `papers`). Must be one of
    /// LEDGER_CATEGORIES.
    #[arg(long)]
    category: String,
    /// Slug. Defaults to a kebab-case derivation of `--title` if omitted.
    #[arg(long)]
    slug: Option<String>,
    /// Frontmatter title.
    #[arg(long)]
    title: String,
    /// Frontmatter status. `draft` or `review`.
    #[arg(long, default_value = "draft")]
    status: String,
    /// Frontmatter priority (`low`, `med`, `high`). Omitted if absent.
    #[arg(long)]
    priority: Option<String>,
    /// Frontmatter tags, comma-separated. Empty / absent → no tags.
    #[arg(long, default_value = "")]
    tags: String,
    /// Modified date, `YYYY-MM-DD`. Defaults to today.
    #[arg(long)]
    modified: Option<String>,
    /// Path to a file containing the markdown body, or `-` to read from stdin.
    #[arg(long)]
    body: String,
}

pub(super) fn add(root: &Path, args: AddArgs) -> CliResult {
    add::add(
        root,
        AddOptions {
            category: args.category,
            slug: args.slug,
            title: args.title,
            status: args.status,
            priority: args.priority,
            tags: args.tags,
            modified: args.modified,
            body: args.body,
        },
    )
}
