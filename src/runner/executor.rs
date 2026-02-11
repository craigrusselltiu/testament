use std::path::Path;
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use crate::parser::{parse_trx, TestResult};

pub enum ExecutorEvent {
    OutputLine(String),
    BuildCompleted(bool),
    Completed(Vec<TestResult>),
    Error(String),
}

pub struct TestExecutor {
    project_path: std::path::PathBuf,
}

impl TestExecutor {
    pub fn new(project_path: &Path) -> Self {
        Self {
            project_path: project_path.to_path_buf(),
        }
    }

    pub fn build(&self) -> Receiver<ExecutorEvent> {
        let (tx, rx) = mpsc::channel();
        let project_path = self.project_path.clone();

        thread::spawn(move || {
            let project_dir = project_path.parent().unwrap_or(Path::new("."));
            let output = match Command::new("dotnet")
                .args(["build", "--verbosity", "minimal"])
                .arg(&project_path)
                .current_dir(project_dir)
                .output()
            {
                Ok(output) => output,
                Err(e) => {
                    let _ = tx.send(ExecutorEvent::Error(format!("Failed to start dotnet: {}", e)));
                    let _ = tx.send(ExecutorEvent::BuildCompleted(false));
                    return;
                }
            };

            let success = output.status.success();
            if !success {
                // Show build errors
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines().chain(stderr.lines()) {
                    if !line.trim().is_empty() {
                        let _ = tx.send(ExecutorEvent::OutputLine(line.to_string()));
                    }
                }
            }

            let _ = tx.send(ExecutorEvent::BuildCompleted(success));
        });

        rx
    }

    pub fn run(&self, test_filter: Option<Vec<String>>) -> Receiver<ExecutorEvent> {
        let (tx, rx) = mpsc::channel();
        let project_path = self.project_path.clone();

        thread::spawn(move || {
            let project_dir = project_path.parent().unwrap_or(Path::new("."));
            // Unique TRX path per run to avoid stale results from crashes
            let trx_path = std::env::temp_dir().join(format!(
                "testament_{}_{}.trx",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
            // Remove any stale file
            let _ = std::fs::remove_file(&trx_path);

            let mut cmd = Command::new("dotnet");
            cmd.args([
                "test",
                "--no-build",
                "--logger",
                &format!("trx;LogFileName={}", trx_path.display()),
                "--verbosity",
                "minimal",
            ]);

            // Add filter if specific tests are selected
            if let Some(tests) = test_filter {
                if !tests.is_empty() {
                    // Strip parameterized test arguments (everything in parentheses)
                    // to avoid special character issues with MSBuild
                    let filter = tests
                        .iter()
                        .map(|t| {
                            let base_name = t.split('(').next().unwrap_or(t);
                            format!("FullyQualifiedName~{}", base_name)
                        })
                        .collect::<Vec<_>>()
                        .join("|");
                    cmd.args(["--filter", &filter]);
                }
            }

            cmd.arg(&project_path);
            cmd.current_dir(project_dir);

            // Log the command for diagnostics
            let cmd_display = format!(
                "dotnet test {} {}",
                project_path.display(),
                if cmd.get_args().any(|a| a.to_string_lossy().contains("--filter")) { "(with filter)" } else { "" }
            );
            let _ = tx.send(ExecutorEvent::OutputLine(format!("> {}", cmd_display)));

            let mut child = match cmd
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(child) => child,
                Err(e) => {
                    let _ = tx.send(ExecutorEvent::Error(format!("Failed to start dotnet: {}", e)));
                    return;
                }
            };

            // Stream stdout, filtering build noise
            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(|l| l.ok()) {
                    if should_show_line(&line)
                        && tx.send(ExecutorEvent::OutputLine(line)).is_err() {
                            return;
                        }
                }
            }

            // Wait for completion
            let status = child.wait();

            // Parse TRX results
            match std::fs::read_to_string(&trx_path) {
                Ok(content) => match parse_trx(&content) {
                    Ok(results) => {
                        let _ = tx.send(ExecutorEvent::Completed(results));
                    }
                    Err(e) => {
                        let _ = tx.send(ExecutorEvent::Error(format!("TRX parse error: {}", e)));
                    }
                },
                Err(_) => {
                    // TRX file not created - dotnet test likely failed before producing results
                    let mut msg = String::from("dotnet test did not produce results.");
                    if let Ok(s) = &status {
                        if !s.success() {
                            msg.push_str(&format!(" Exit code: {}", s.code().unwrap_or(-1)));
                        }
                    }
                    let _ = tx.send(ExecutorEvent::Error(msg));
                }
            }

            // Cleanup
            let _ = std::fs::remove_file(&trx_path);
        });

        rx
    }
}

/// Filter out verbose build output, keeping only test-relevant lines
fn should_show_line(line: &str) -> bool {
    let trimmed = line.trim();

    // Skip empty lines
    if trimmed.is_empty() {
        return false;
    }

    // Skip common MSBuild/dotnet noise
    let skip_patterns = [
        "Build started",
        "Build succeeded",
        "Determining projects to restore",
        "Restored ",
        "Nothing to do",
        "Time Elapsed",
        "Microsoft (R) Test Execution Command Line Tool",
        "Copyright (C) Microsoft",
        "Starting test execution",
        "A total of ",
        "Results File:",
        "All projects are up-to-date",
        "up-to-date for restore",
        "NuGet.targets",
        "Test run for ",
        "VSTest",
        "Attachments:",
        // xUnit specific noise
        "[xUnit.net ",
        "Error Message:",
        "Stack Trace:",
        "Expected:",
        "Actual:",
        "Assert.",
    ];

    for pattern in skip_patterns {
        if trimmed.contains(pattern) {
            return false;
        }
    }

    // Skip lines that look like project paths being built
    if trimmed.ends_with(".csproj") || trimmed.ends_with(".sln") {
        return false;
    }

    // Skip build output paths (e.g., "Example -> E:\...\Example.dll")
    if trimmed.contains(" -> ") && trimmed.ends_with(".dll") {
        return false;
    }

    // Skip stack trace lines
    if trimmed.starts_with("at ") || trimmed.starts_with("--- ") {
        return false;
    }

    // Skip lines that are just file paths with line numbers (stack traces)
    if trimmed.contains(":line ") {
        return false;
    }

    // Skip "Skipped" lines from test output (detailed skip info)
    if trimmed.starts_with("Skipped ") {
        return false;
    }

    true
}
