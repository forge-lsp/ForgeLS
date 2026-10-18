use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "fls", about = "ForgeLS linter CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Lint Solidity files.
    Check(CheckArgs),
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

pub fn parse() -> Cli {
    Cli::parse()
}
