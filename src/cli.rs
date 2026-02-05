use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "testament")]
#[command(about = "A TUI for discovering, running, and monitoring .NET tests")]
#[command(version)]
pub struct Cli {
    /// Path to solution file, project file, or directory containing one
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Run tests (default if no subcommand given)
    Run {
        /// Filter tests by name pattern
        #[arg(short, long)]
        filter: Option<String>,
    },
    /// Run tests changed in a GitHub pull request
    Pr {
        /// GitHub PR URL (e.g., https://github.com/owner/repo/pull/123)
        #[arg(value_name = "URL")]
        url: String,

        /// Path to solution file, project file, or directory containing one
        #[arg(short, long, value_name = "PATH")]
        path: Option<PathBuf>,

        /// Run tests directly without launching the TUI
        #[arg(long)]
        no_tui: bool,
    },
}

impl Cli {
    pub fn parse_args() -> Self {
        Cli::parse()
    }
}
