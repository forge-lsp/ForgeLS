use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "fls",
    version,
    about = "ForgeLS entrypoint: lsp server plus check linter"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the LSP server on stdio.
    Lsp(LspArgs),
    /// Lint Solidity files.
    Check(CheckArgs),
    /// Format Solidity files (V1: trim plus spacing plus 120-column wrap).
    Format(FormatArgs),
    /// Scaffold ./fls.json with default lint levels.
    Init(InitArgs),
}

#[derive(Debug, Args)]
pub struct LspArgs {
    /// Log level: trace, debug, info, warn, error.
    #[arg(long, value_name = "LEVEL")]
    pub log_level: Option<String>,
}

#[derive(Debug, Args)]
pub struct CheckArgs {
    /// Files or dirs to lint. Empty means scan cwd for *.sol.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,
    /// Apply safe fixes in place.
    #[arg(long)]
    pub write: bool,
    /// Emit JSON report.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct FormatArgs {
    /// Files or dirs to format. Empty means scan cwd for *.sol.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,
    /// Write formatted text back in place.
    #[arg(long)]
    pub write: bool,
    /// List files that would change, no writes.
    #[arg(long)]
    pub check: bool,
}

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Overwrite an existing fls.json.
    #[arg(long)]
    pub force: bool,
}

pub fn parse() -> Cli {
    Cli::parse()
}
