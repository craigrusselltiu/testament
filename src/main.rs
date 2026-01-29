mod cli;
mod error;

use cli::Cli;

fn main() {
    let cli = Cli::parse_args();

    match &cli.command {
        Some(cli::Command::Run { filter }) => {
            println!("Running tests...");
            if let Some(f) = filter {
                println!("Filter: {}", f);
            }
        }
        None => {
            println!("Testament - .NET Test Runner TUI");
            println!("Use --help for usage information");
        }
    }
}
