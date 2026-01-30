use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::model::{TestProject, TestStatus};
use crate::parser::TestOutcome;
use crate::runner::{ExecutorEvent, FileWatcher, TestExecutor};
use crate::ui::{self, build_test_items, layout::{AppState, startup_art, random_ready_phrase}, Pane, TestListItem};

pub fn run(projects: Vec<TestProject>, solution_dir: PathBuf) -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new(projects);
    state.output = format!("{}\n{}", startup_art(), random_ready_phrase());

    let mut executor_rx: Option<std::sync::mpsc::Receiver<ExecutorEvent>> = None;
    let mut file_watcher: Option<FileWatcher> = None;

    // Main loop
    loop {
        terminal.draw(|f| ui::draw(f, &mut state))?;

        // Check for file changes in watch mode
        if state.watch_mode {
            if let Some(ref watcher) = file_watcher {
                if watcher.try_recv() && executor_rx.is_none() {
                    state.output.push_str("\n[Watch] File change detected, running tests...\n");
                    run_tests(&mut state, &mut executor_rx);
                }
            }
        }

        // Check for executor events
        if let Some(ref rx) = executor_rx {
            while let Ok(event) = rx.try_recv() {
                match event {
                    ExecutorEvent::OutputLine(line) => {
                        let trimmed = line.trim();
                        if trimmed.starts_with("Passed") || trimmed.starts_with("Failed") {
                            // Increment progress instead of appending
                            if let Some((completed, _)) = &mut state.test_progress {
                                *completed += 1;
                            }
                        } else {
                            state.output.push('\n');
                            state.output.push_str(&line);
                        }
                    }
                    ExecutorEvent::Status(msg) => {
                        state.output.push('\n');
                        state.output.push_str(&msg);
                    }
                    ExecutorEvent::BuildCompleted(success) => {
                        if success {
                            state.output.push_str("\nBuild succeeded");
                        } else {
                            state.output.push_str("\nBuild FAILED");
                        }
                        executor_rx = None;
                        break;
                    }
                    ExecutorEvent::Completed(results) => {
                        // Track failed tests
                        state.last_failed.clear();
                        for result in &results {
                            if result.outcome == TestOutcome::Failed {
                                state.last_failed.insert(result.test_name.clone());
                            }
                        }

                        apply_results(&mut state, &results);
                        state.test_progress = None;
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

                // Handle filter mode input
                if state.filter_active {
                    match key.code {
                        KeyCode::Esc => {
                            state.filter_active = false;
                        }
                        KeyCode::Enter => {
                            state.filter_active = false;
                        }
                        KeyCode::Backspace => {
                            state.filter.pop();
                        }
                        KeyCode::Char(c) => {
                            state.filter.push(c);
                        }
                        _ => {}
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Down => {
                        move_selection(&mut state, 1);
                    }
                    KeyCode::Up => {
                        move_selection(&mut state, -1);
                    }
                    KeyCode::Tab => {
                        state.active_pane = match state.active_pane {
                            Pane::Projects => Pane::Tests,
                            Pane::Tests => Pane::Output,
                            Pane::Output => Pane::Projects,
                        };
                    }
                    KeyCode::BackTab => {
                        state.active_pane = match state.active_pane {
                            Pane::Projects => Pane::Output,
                            Pane::Tests => Pane::Projects,
                            Pane::Output => Pane::Tests,
                        };
                    }
                    KeyCode::Char(' ') => {
                        if state.active_pane == Pane::Tests {
                            toggle_space_action(&mut state);
                        }
                    }
                    KeyCode::Char('x') => {
                        state.output.clear();
                        state.output_scroll = 0;
                        state.test_progress = None;
                    }
                    KeyCode::Char('c') => {
                        state.clear_selection();
                    }
                    KeyCode::Char('/') => {
                        state.filter_active = true;
                        state.filter.clear();
                    }
                    KeyCode::Esc => {
                        if !state.filter.is_empty() {
                            state.filter.clear();
                        }
                    }
                    KeyCode::Char('w') => {
                        state.watch_mode = !state.watch_mode;
                        if state.watch_mode {
                            match FileWatcher::new(&solution_dir) {
                                Ok(watcher) => {
                                    file_watcher = Some(watcher);
                                    state.output.push_str("\n[Watch] Watch mode enabled\n");
                                }
                                Err(e) => {
                                    state.watch_mode = false;
                                    state.output.push_str(&format!(
                                        "\n[Watch] Failed to enable watch mode: {}\n",
                                        e
                                    ));
                                }
                            }
                        } else {
                            file_watcher = None;
                            state.output.push_str("\n[Watch] Watch mode disabled\n");
                        }
                    }
                    KeyCode::Char('r') => {
                        if executor_rx.is_none() {
                            run_tests(&mut state, &mut executor_rx);
                        }
                    }
                    KeyCode::Char('b') => {
                        if executor_rx.is_none() {
                            build_project(&mut state, &mut executor_rx);
                        }
                    }
                    KeyCode::Char('a') => {
                        if executor_rx.is_none() && !state.last_failed.is_empty() {
                            run_failed_tests(&mut state, &mut executor_rx);
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
    match state.active_pane {
        Pane::Projects => {
            let len = state.projects.len();
            if len == 0 {
                return;
            }
            let current = state.project_state.selected().unwrap_or(0) as i32;
            let new = (current + delta).rem_euclid(len as i32) as usize;
            state.project_state.select(Some(new));
        }
        Pane::Tests => {
            if let Some(idx) = state.project_state.selected() {
                if let Some(project) = state.projects.get(idx) {
                    let items = build_test_items(
                        &project.classes,
                        &state.collapsed_classes,
                        &state.filter,
                    );
                    let len = items.len();
                    if len == 0 {
                        return;
                    }
                    let current = state.test_state.selected().unwrap_or(0) as i32;
                    let new = (current + delta).rem_euclid(len as i32) as usize;
                    state.test_state.select(Some(new));
                }
            }
        }
        Pane::Output => {
            let line_count = state.output.lines().count() as i32;
            let current = state.output_scroll as i32;
            let new = (current + delta).max(0).min(line_count.saturating_sub(1));
            state.output_scroll = new as u16;
        }
    }
}

fn toggle_collapse(state: &mut AppState) {
    if let Some(idx) = state.project_state.selected() {
        if let Some(project) = state.projects.get(idx) {
            let items =
                build_test_items(&project.classes, &state.collapsed_classes, &state.filter);
            if let Some(selected) = state.test_state.selected() {
                if let Some(TestListItem::Class(class_name)) = items.get(selected) {
                    state.toggle_class_collapsed(class_name);
                }
            }
        }
    }
}

fn toggle_space_action(state: &mut AppState) {
    if let Some(idx) = state.project_state.selected() {
        if let Some(project) = state.projects.get(idx) {
            let items =
                build_test_items(&project.classes, &state.collapsed_classes, &state.filter);
            if let Some(selected) = state.test_state.selected() {
                if let Some(item) = items.get(selected) {
                    match item {
                        TestListItem::Class(_) => toggle_collapse(state),
                        TestListItem::Test(test_name) => state.toggle_test_selected(test_name),
                    }
                }
            }
        }
    }
}

fn run_tests(
    state: &mut AppState,
    executor_rx: &mut Option<std::sync::mpsc::Receiver<ExecutorEvent>>,
) {
    if let Some(idx) = state.project_state.selected() {
        // Get path and test count before mutating
        let (path, project_test_count) = if let Some(project) = state.projects.get(idx) {
            (project.path.clone(), project.test_count())
        } else {
            return;
        };

        // Count total tests for progress tracking
        let total_tests = if state.selected_tests.is_empty() {
            mark_all_tests_running(state);
            project_test_count
        } else {
            mark_selected_tests_running(state);
            state.selected_tests.len()
        };

        state.output.push_str("\n────────────────────────────\n");
        state.output.push_str("Running tests...\n");
        state.test_progress = Some((0, total_tests));

        let executor = TestExecutor::new(&path);
        *executor_rx = Some(executor.run());
    }
}

fn build_project(
    state: &mut AppState,
    executor_rx: &mut Option<std::sync::mpsc::Receiver<ExecutorEvent>>,
) {
    if let Some(idx) = state.project_state.selected() {
        if let Some(project) = state.projects.get(idx) {
            let path = project.path.clone();

            state.output.push_str("\n────────────────────────────\n");
            state.output.push_str("Building...\n");

            let executor = TestExecutor::new(&path);
            *executor_rx = Some(executor.build());
        }
    }
}

fn run_failed_tests(
    state: &mut AppState,
    executor_rx: &mut Option<std::sync::mpsc::Receiver<ExecutorEvent>>,
) {
    if let Some(idx) = state.project_state.selected() {
        if let Some(project) = state.projects.get(idx) {
            let path = project.path.clone();

            // Mark failed tests as running
            let failed_count = state.last_failed.len();
            if let Some(project) = state.projects.get_mut(idx) {
                for class in &mut project.classes {
                    for test in &mut class.tests {
                        if state.last_failed.contains(&test.full_name)
                            || state.last_failed.iter().any(|f| f.ends_with(&test.name))
                        {
                            test.status = TestStatus::Running;
                        }
                    }
                }
            }

            state.output.push_str("\n────────────────────────────\n");
            state.output.push_str(&format!("Re-running {} failed...\n", failed_count));
            state.test_progress = Some((0, failed_count));

            let executor = TestExecutor::new(&path);
            *executor_rx = Some(executor.run());
        }
    }
}

fn mark_all_tests_running(state: &mut AppState) {
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

fn mark_selected_tests_running(state: &mut AppState) {
    if let Some(idx) = state.project_state.selected() {
        if let Some(project) = state.projects.get_mut(idx) {
            for class in &mut project.classes {
                for test in &mut class.tests {
                    if state.selected_tests.contains(&test.full_name) {
                        test.status = TestStatus::Running;
                    }
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
