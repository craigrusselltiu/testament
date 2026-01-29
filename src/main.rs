mod cli;
mod error;
mod model;
mod runner;

use std::env;

use cli::Cli;
use runner::{discover_projects, find_solution};

fn main() {
    let cli = Cli::parse_args();

    let start_dir = cli.path.unwrap_or_else(|| env::current_dir().unwrap());

    match find_solution(&start_dir) {
        Ok(sln_path) => {
            println!("Found solution: {}", sln_path.display());

            match discover_projects(&sln_path) {
                Ok(projects) => {
                    if projects.is_empty() {
                        println!("No test projects found.");
                    } else {
                        for project in &projects {
                            println!("\n{} ({} tests)", project.name, project.test_count());
                            for class in &project.classes {
                                println!("  {}", class.full_name());
                                for test in &class.tests {
                                    println!("    - {}", test.name);
                                }
                            }
                        }
                    }
                }
                Err(e) => eprintln!("Failed to discover tests: {}", e),
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    if let Some(cli::Command::Run { filter }) = &cli.command {
        println!("\nRunning tests...");
        if let Some(f) = filter {
            println!("Filter: {}", f);
        }
    }
}
