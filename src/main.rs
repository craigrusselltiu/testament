mod app;
mod cli;
mod error;
mod git;
mod model;
mod parser;
mod runner;
mod ui;

use std::env;
use std::process::Command;

use cli::{Cli, Command as CliCommand};
use git::{extract_changed_tests, fetch_pr_diff, get_github_token, parse_pr_url};
use runner::{discover_projects_lazy, discover_projects_from_paths, find_solution};

fn main() {
    let cli = Cli::parse_args();

    // Handle PR subcommand
    if let Some(CliCommand::Pr { url, path, no_tui }) = cli.command {
        run_pr_mode(&url, path, no_tui);
        return;
    }

    // Normal TUI mode
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
    
    // Build context string from solution/project name
    let context = sln_path.file_name()
        .and_then(|n| n.to_str())
        .map(|name| format!("Running Tests for Solution: {}", name));

    if let Err(e) = app::run(projects, solution_dir, discovery_rx, context) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_pr_mode(url: &str, path: Option<std::path::PathBuf>, no_tui: bool) {
    println!("Fetching PR: {}", url);

    // Parse PR URL
    let pr_info = match parse_pr_url(url) {
        Ok(info) => info,
        Err(e) => {
            eprintln!("Error parsing PR URL: {}", e);
            std::process::exit(1);
        }
    };

    println!("PR #{} in {}/{}", pr_info.number, pr_info.owner, pr_info.repo);

    // Get GitHub token
    let token = get_github_token();
    if token.is_none() {
        eprintln!("Warning: No GitHub token found. Set GITHUB_TOKEN or use `gh auth login`");
    }

    // Fetch PR diff
    let diff = match fetch_pr_diff(&pr_info, token.as_deref()) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error fetching PR: {}", e);
            std::process::exit(1);
        }
    };

    // Extract changed tests from diff
    let changed_tests = extract_changed_tests(&diff);

    if changed_tests.is_empty() {
        println!("No test changes detected in this PR.");
        return;
    }

    println!("\nFound {} changed test(s):", changed_tests.len());
    for test in &changed_tests {
        println!("  - {} ({})", test.method_name, test.file_path);
    }

    // Find project files from test paths
    let repo_root = path.clone().unwrap_or_else(|| env::current_dir().unwrap());
    let mut project_paths: Vec<std::path::PathBuf> = Vec::new();
    
    for test in &changed_tests {
        let test_file = repo_root.join(&test.file_path);
        if let Some(proj) = find_csproj_for_file(&test_file) {
            if !project_paths.contains(&proj) {
                project_paths.push(proj);
            }
        }
    }

    if project_paths.is_empty() {
        eprintln!("Error: Could not find any .csproj files for the changed tests.");
        eprintln!("Make sure you're running from the repository root.");
        std::process::exit(1);
    }

    // Collect test method names for pre-selection
    let test_names: Vec<String> = changed_tests.iter().map(|t| t.method_name.clone()).collect();

    if no_tui {
        // --no-tui flag: run tests directly without TUI
        println!("\nRunning tests in {} project(s):", project_paths.len());
        for proj in &project_paths {
            println!("  - {}", proj.display());
        }

        let filter_parts: Vec<String> = changed_tests
            .iter()
            .map(|t| format!("FullyQualifiedName~{}", t.method_name))
            .collect();
        let filter = filter_parts.join("|");

        println!("\nFilter: {}", filter);
        println!("----------------------------------------");

        // Run dotnet test for each project
        for proj in &project_paths {
            println!("\nRunning: dotnet test {}", proj.display());
            let status = Command::new("dotnet")
                .args([
                    "test",
                    proj.to_str().unwrap(),
                    "--no-build",
                    "--filter",
                    &filter,
                ])
                .status();

            match status {
                Ok(s) => {
                    if !s.success() {
                        std::process::exit(s.code().unwrap_or(1));
                    }
                }
                Err(e) => {
                    eprintln!("Failed to run dotnet test: {}", e);
                    std::process::exit(1);
                }
            }
        }
    } else {
        // Launch TUI with only the changed projects (not all projects in solution)
        let start_dir = path.unwrap_or_else(|| env::current_dir().unwrap());
        
        let (projects, discovery_rx) = match discover_projects_from_paths(project_paths.clone()) {
            Ok(result) => result,
            Err(e) => {
                eprintln!("Failed to discover projects: {}", e);
                std::process::exit(1);
            }
        };

        // Use the first project's parent or start_dir as solution_dir
        let solution_dir = project_paths.first()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or(start_dir);

        let context = Some(format!("Running Tests for PR #{}", pr_info.number));

        if let Err(e) = app::run_with_preselected(projects, solution_dir, discovery_rx, test_names, context) {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

/// Find the .csproj file for a given source file by searching parent directories
fn find_csproj_for_file(file_path: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut dir = file_path.parent()?;
    
    loop {
        // Look for .csproj files in this directory
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "csproj") {
                    return Some(path);
                }
            }
        }
        
        // Move to parent directory
        dir = dir.parent()?;
    }
}
