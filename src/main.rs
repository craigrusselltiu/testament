mod app;
mod cli;
mod error;
mod model;
mod parser;
mod runner;
mod ui;

use std::env;

use cli::Cli;
use runner::{discover_projects, find_solution};

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

    let projects = match discover_projects(&sln_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to discover tests: {}", e);
            std::process::exit(1);
        }
    };

    if let Err(e) = app::run(projects) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
