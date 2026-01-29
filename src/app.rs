use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::model::{TestProject, TestStatus};
use crate::parser::TestOutcome;
use crate::runner::{ExecutorEvent, TestExecutor};
use crate::ui::{self, layout::AppState};

pub fn run(projects: Vec<TestProject>) -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new(projects);
    state.output = "Press 'r' to run tests, 'q' to quit, j/k to navigate".to_string();

    let mut executor_rx: Option<std::sync::mpsc::Receiver<ExecutorEvent>> = None;

    // Main loop
    loop {
        terminal.draw(|f| ui::draw(f, &mut state))?;

        // Check for executor events
        if let Some(ref rx) = executor_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    ExecutorEvent::OutputLine(line) => {
                        state.output.push('\n');
                        state.output.push_str(&line);
                    }
                    ExecutorEvent::Completed(results) => {
                        apply_results(&mut state, &results);
                        let passed = results.iter().filter(|r| r.outcome == TestOutcome::Passed).count();
                        let failed = results.iter().filter(|r| r.outcome == TestOutcome::Failed).count();
                        state.output.push_str(&format!(
                            "\n\n--- Results: {} passed, {} failed ---",
                            passed, failed
                        ));
                        executor_rx = None;
                        break;
                    }
                    ExecutorEvent::Error(e) => {
                        state.output.push_str(&format!("\nError: {}", e));
                        executor_rx = None;
                        break;
                    }
                }
            }
        }

        // Poll for keyboard input with timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('j') | KeyCode::Down => {
                        move_selection(&mut state, 1);
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        move_selection(&mut state, -1);
                    }
                    KeyCode::Tab => {
                        // TODO: Switch focus between panes
                    }
                    KeyCode::Char('r') => {
                        if executor_rx.is_none() {
                            if let Some(idx) = state.project_state.selected() {
                                if let Some(project) = state.projects.get(idx) {
                                    let name = project.name.clone();
                                    let path = project.path.clone();
                                    mark_tests_running(&mut state);
                                    state.output = format!("Running tests for {}...\n", name);
                                    let executor = TestExecutor::new(&path);
                                    executor_rx = Some(executor.run());
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn move_selection(state: &mut AppState, delta: i32) {
    let len = state.projects.len();
    if len == 0 {
        return;
    }
    let current = state.project_state.selected().unwrap_or(0) as i32;
    let new = (current + delta).rem_euclid(len as i32) as usize;
    state.project_state.select(Some(new));
}

fn mark_tests_running(state: &mut AppState) {
    if let Some(idx) = state.project_state.selected() {
        if let Some(project) = state.projects.get_mut(idx) {
            for class in &mut project.classes {
                for test in &mut class.tests {
                    test.status = TestStatus::Running;
                }
            }
        }
    }
}

fn apply_results(state: &mut AppState, results: &[crate::parser::TestResult]) {
    if let Some(idx) = state.project_state.selected() {
        if let Some(project) = state.projects.get_mut(idx) {
            for class in &mut project.classes {
                for test in &mut class.tests {
                    // Find matching result by test name
                    if let Some(result) = results.iter().find(|r| {
                        r.test_name == test.full_name || r.test_name.ends_with(&test.name)
                    }) {
                        test.status = match result.outcome {
                            TestOutcome::Passed => TestStatus::Passed,
                            TestOutcome::Failed => TestStatus::Failed,
                            TestOutcome::Skipped => TestStatus::Skipped,
                        };
                        test.duration_ms = Some(result.duration_ms);
                        test.error_message = result.error_message.clone();
                    }
                }
            }
        }
    }
}
