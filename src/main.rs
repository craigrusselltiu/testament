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
use ui::layout::random_startup_phrase;

fn main() {
    let cli = Cli::parse_args();

    // Show loading phrase before discovering projects
    println!("{}", random_startup_phrase());

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

    let solution_dir = sln_path.parent().unwrap_or(&start_dir).to_path_buf();

    if let Err(e) = app::run(projects, solution_dir) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
