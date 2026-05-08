use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "rag", version, about = "Vault-based RAG indexer & search", long_about = None)]
pub struct Cli {
    /// Override walk-up vault discovery
    #[arg(long, global = true)]
    pub vault: Option<PathBuf>,

    /// Emit JSON output to stdout
    #[arg(long, global = true)]
    pub json: bool,

    /// Suppress non-error output
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Additional human-readable detail
    #[arg(long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create a new vault
    Init(InitCmd),
    /// Register files with the vault
    Add(AddCmd),
    /// Deregister files
    Rm(RmCmd),
    /// Remove registry rows in non-`indexed` states
    Prune(PruneCmd),
    /// List registered files
    Ls(LsCmd),
    /// Report vault state
    Status(StatusCmd),
    /// Process the registry
    Index(IndexCmd),
    /// Hybrid retrieval over the index
    Search(SearchCmd),
    /// Display a chunk or file
    Show(ShowCmd),
    /// Read or modify vault settings
    Config(ConfigCmd),
    /// Vault metadata and statistics
    Info(InfoCmd),
}

#[derive(Debug, Args)]
pub struct InitCmd {
    /// Vault directory (defaults to cwd)
    pub directory: Option<PathBuf>,
    /// Proceed even if .vault/ already exists
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct AddCmd {
    /// Files or directories to register
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,
    /// Skip files whose extension is not supported
    #[arg(long)]
    pub skip_unsupported: bool,
    /// Ignore .vaultignore for this invocation
    #[arg(long)]
    pub no_ignore: bool,
    /// Ignore both .vaultignore and built-in defaults
    #[arg(long)]
    pub force: bool,
    /// Walk and report; make no changes
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct RmCmd {
    /// Paths to remove
    pub paths: Vec<PathBuf>,
    /// Deregister every file in the registry
    #[arg(long, conflicts_with = "paths")]
    pub all: bool,
    /// Skip the interactive confirmation when using --all
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct PruneCmd {
    /// Target a specific status (default: missing)
    #[arg(long)]
    pub status: Option<String>,
    /// Delete every row whose status is not `indexed`
    #[arg(long)]
    pub all_non_indexed: bool,
    /// Report what would be deleted
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct LsCmd {
    /// Filter to a specific status
    #[arg(long)]
    pub status: Option<String>,
}

#[derive(Debug, Args)]
pub struct StatusCmd {
    /// Detail view of one status only
    #[arg(long)]
    pub filter: Option<String>,
    /// List all files in each section instead of truncating
    #[arg(long)]
    pub full: bool,
    /// Walk vault directory, report files on disk not in registry
    #[arg(long)]
    pub show_untracked: bool,
    /// Skip the modification check
    #[arg(long)]
    pub no_stat: bool,
}

#[derive(Debug, Args)]
pub struct IndexCmd {
    /// Reprocess all `indexed` files regardless of mtime/size
    #[arg(long)]
    pub force: bool,
    /// Include files in `failed` state in the processing pass
    #[arg(long)]
    pub retry_failed: bool,
    /// Restrict processing to specific paths (must already be registered)
    #[arg(long)]
    pub paths: Vec<PathBuf>,
    /// Exit immediately if the lock is held
    #[arg(long)]
    pub no_wait: bool,
    /// Wait up to N seconds for the lock
    #[arg(long, default_value = "60")]
    pub wait: u64,
}

#[derive(Debug, Args)]
pub struct SearchCmd {
    /// Free-text query
    pub query: String,
    /// Number of results
    #[arg(long)]
    pub k: Option<u32>,
    /// Restrict to files matching a glob pattern
    #[arg(long)]
    pub filter: Option<String>,
    /// Bypass fusion: vector retrieval only
    #[arg(long, conflicts_with = "keyword_only")]
    pub vector_only: bool,
    /// Bypass fusion: keyword retrieval only
    #[arg(long)]
    pub keyword_only: bool,
    /// Minimum fused score
    #[arg(long)]
    pub threshold: Option<f32>,
}

#[derive(Debug, Args)]
pub struct ShowCmd {
    /// Chunk ID or file path
    pub target: String,
}

#[derive(Debug, Args)]
pub struct ConfigCmd {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Print current effective value
    Get { key: String },
    /// Validate, write, update timestamp
    Set { key: String, value: String },
    /// Delete the row, revert to default
    Unset { key: String },
    /// Enumerate settings
    List {
        /// Show only keys with set values
        #[arg(long)]
        modified: bool,
        /// Show built-in defaults regardless of vault state
        #[arg(long)]
        defaults: bool,
    },
}

#[derive(Debug, Args)]
pub struct InfoCmd {
    /// Run consistency checks
    #[arg(long)]
    pub check: bool,
}
