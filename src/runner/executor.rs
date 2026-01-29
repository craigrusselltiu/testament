use std::path::Path;
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use crate::error::{Result, TestamentError};
use crate::parser::{parse_trx, TestResult};

pub enum ExecutorEvent {
    OutputLine(String),
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

    pub fn run(&self) -> Receiver<ExecutorEvent> {
        let (tx, rx) = mpsc::channel();
        let project_path = self.project_path.clone();

        thread::spawn(move || {
            let trx_path = std::env::temp_dir().join("testament_results.trx");

            let mut child = match Command::new("dotnet")
                .args([
                    "test",
                    "--logger",
                    &format!("trx;LogFileName={}", trx_path.display()),
                    "--verbosity",
                    "normal",
                ])
                .arg(&project_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(child) => child,
                Err(e) => {
                    let _ = tx.send(ExecutorEvent::Error(format!("Failed to start dotnet: {}", e)));
                    return;
                }
            };

            // Stream stdout
            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(|l| l.ok()) {
                    if tx.send(ExecutorEvent::OutputLine(line)).is_err() {
                        return;
                    }
                }
            }

            // Wait for completion
            let _ = child.wait();

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
                Err(e) => {
                    let _ = tx.send(ExecutorEvent::Error(format!("Failed to read TRX: {}", e)));
                }
            }

            // Cleanup
            let _ = std::fs::remove_file(&trx_path);
        });

        rx
    }
}

pub fn run_tests(project_path: &Path) -> Result<Vec<TestResult>> {
    let trx_path = std::env::temp_dir().join("testament_results.trx");

    let output = Command::new("dotnet")
        .args([
            "test",
            "--logger",
            &format!("trx;LogFileName={}", trx_path.display()),
        ])
        .arg(project_path)
        .output()
        .map_err(|e| TestamentError::DotnetExecution(e.to_string()))?;

    if !output.status.success() {
        // Tests may have failed, but we still want to parse results
    }

    let content = std::fs::read_to_string(&trx_path).map_err(|e| {
        TestamentError::TrxParse(format!("Failed to read TRX file: {}", e))
    })?;

    let results = parse_trx(&content)?;

    let _ = std::fs::remove_file(&trx_path);

    Ok(results)
}
