mod app;
mod cli;
mod error;
mod model;
mod parser;
mod runner;
mod ui;

use std::env;

use cli::Cli;
use runner::{discover_projects_lazy, find_solution};

fn main() {
    let cli = Cli::parse_args();

    let start_dir = cli.path.unwrap_or_else(|| env::current_dir().unwrap());

    let sln_path = match find_solution(&start_dir) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let (projects, discovery_rx) = match discover_projects_lazy(&sln_path) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Failed to discover projects: {}", e);
            std::process::exit(1);
        }
    };

    let solution_dir = sln_path.parent().unwrap_or(&start_dir).to_path_buf();

    if let Err(e) = app::run(projects, solution_dir, discovery_rx) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
