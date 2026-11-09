//! Command-line surface. Types are declarative; behaviour lives in `cmd::*`.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "tatrs",
    version,
    about = "Git-native task tracker for humans and agents",
    long_about = "A task database for repositories: every task is tasks/<HUID>/TASK.md.\n\
                  Machine-readable with --json, non-interactive by default, atomic on disk."
)]
pub struct Cli {
    #[command(flatten)]
    pub global: Global,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Args)]
pub struct Global {
    /// Machine-readable JSON output (schema 1)
    #[arg(long, global = true, conflicts_with = "porcelain")]
    pub json: bool,

    /// Tab-separated fields, no decoration
    #[arg(long, global = true)]
    pub porcelain: bool,

    /// Suppress non-essential output
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Run as if started in PATH
    #[arg(long = "cwd", global = true, value_name = "PATH")]
    pub cwd: Option<PathBuf>,

    /// Report malformed entries instead of failing
    #[arg(long, global = true)]
    pub lenient: bool,

    /// Refuse to modify anything
    #[arg(long, global = true)]
    pub read_only: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a tasks/ directory in the current project
    Init,

    /// Create a task
    New(NewArgs),

    /// List tasks
    Ls(LsArgs),

    /// Show a single task, including its body
    Show(ShowArgs),

    /// Validate the tasks/ database
    Check(CheckArgs),

    /// Print version information
    Version,
}

#[derive(Debug, Clone, Args)]
pub struct NewArgs {
    /// Task title (joined with spaces)
    #[arg(value_name = "TITLE")]
    pub title: Vec<String>,

    /// Tags; may be repeated and may contain commas
    #[arg(short = 't', long = "tag", value_name = "TAGS")]
    pub tags: Vec<String>,

    /// Sort priority; higher values sort first
    #[arg(short = 'p', long, default_value_t = tatrs_core::task::DEFAULT_PRIORITY, allow_negative_numbers = true)]
    pub priority: i64,

    /// Deterministic task id; if it already exists this is a no-op (exit 0)
    #[arg(long, value_name = "HUID")]
    pub id: Option<String>,

    /// Optional id suffix, e.g. an agent or branch name
    #[arg(short = 's', long, value_name = "SUFFIX")]
    pub suffix: Option<String>,

    /// Add an empty "## Acceptance" section to the body
    #[arg(long)]
    pub acceptance: bool,

    /// Task body Markdown; a blank line is inserted before it
    #[arg(long, value_name = "MARKDOWN", conflicts_with = "body_file")]
    pub body: Option<String>,

    /// Read the task body from a file ("-" for stdin)
    #[arg(long = "body-file", value_name = "PATH")]
    pub body_file: Option<PathBuf>,

    /// Print only the new task id
    #[arg(long = "print-id", conflicts_with = "json")]
    pub print_id: bool,

    /// Assume "yes" for any confirmation
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Show what would be created without writing anything
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Sort {
    Priority,
    Id,
}

#[derive(Debug, Clone, Args)]
pub struct LsArgs {
    /// TQL query; see docs/QUERY.md (for example `:bug and priority lt 50`)
    #[arg(value_name = "QUERY", allow_negative_numbers = true)]
    pub query: Vec<String>,

    /// Include both open and closed tasks
    #[arg(short = 'a', long)]
    pub all: bool,

    /// Only closed tasks
    #[arg(short = 'c', long = "closed")]
    pub closed: bool,

    /// Only tasks carrying this tag (repeatable, all must match)
    #[arg(short = 't', long = "tag", value_name = "TAG")]
    pub tags: Vec<String>,

    /// Sort order
    #[arg(long, value_enum, value_name = "KEY")]
    pub sort: Option<Sort>,

    /// Apply update actions to matched tasks: close, open, +tag, -tag
    #[arg(short = 'u', long = "update", value_name = "OPS")]
    pub update: Option<String>,

    /// With --update: report what would change without writing anything
    #[arg(long)]
    pub dry_run: bool,

    /// Exit 5 when nothing matches instead of printing an empty list
    #[arg(long)]
    pub fail_if_empty: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ShowArgs {
    /// Task id
    #[arg(value_name = "HUID")]
    pub huid: String,

    /// Print only the path to TASK.md
    #[arg(long)]
    pub path_only: bool,

    /// Print TASK.md exactly as stored
    #[arg(long, conflicts_with = "path_only")]
    pub raw: bool,
}

#[derive(Debug, Clone, Args)]
pub struct CheckArgs {
    /// Treat warnings as failures (use this in CI)
    #[arg(long)]
    pub strict: bool,
}
