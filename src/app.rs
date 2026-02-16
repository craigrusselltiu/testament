use std::collections::HashSet;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::git::ChangedTest;
use crate::model::{TestProject, TestStatus};
use crate::parser::TestOutcome;
use crate::runner::{DiscoveryEvent, ExecutorEvent, FileWatcher, TestExecutor};
use crate::ui::{self, layout::{AppState, startup_art, random_startup_phrase, random_ready_phrase}, Pane, TestListItem};

pub fn run(
    projects: Vec<TestProject>,
    solution_dir: PathBuf,
    discovery_rx: mpsc::Receiver<DiscoveryEvent>,
    context: Option<String>,
) -> io::Result<()> {
    run_with_preselected(projects, solution_dir, discovery_rx, Vec::new(), context)
}

pub fn run_with_preselected(
    projects: Vec<TestProject>,
    solution_dir: PathBuf,
    discovery_rx: mpsc::Receiver<DiscoveryEvent>,
    preselected_tests: Vec<ChangedTest>,
    context: Option<String>,
) -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new(projects);
    state.output = format!("{}\n{}", startup_art(), random_startup_phrase());
    state.discovering = true;
    state.status = "Discovering tests...".to_string();
    state.context = context;
    state.output_auto_scroll = true;
    
    // Store preselected test names to match after discovery
    let preselected = preselected_tests;
    let filter_to_preselected = !preselected.is_empty();

    let mut executor_rx: Option<mpsc::Receiver<ExecutorEvent>> = None;
    let mut file_watcher: Option<FileWatcher> = None;

    // Main loop
    loop {
        // Check for discovery events (background test discovery)
        if state.discovering {
            while let Ok(event) = discovery_rx.try_recv() {
                state.dirty = true;
                match event {
                    DiscoveryEvent::ProjectDiscovered(idx, classes) => {
                        // Filter to only preselected tests if in PR mode
                        let classes = if filter_to_preselected {
                            let filtered = filter_classes_to_tests(&classes, &preselected);
                            if filtered.iter().map(|c| c.tests.len()).sum::<usize>() > 0 {
                                filtered
                            } else {
                                // PR tests not found in discovery (new tests not in current branch).
                                // Only create synthetic entries for tests that belong to this project.
                                let project_dir = state.projects.get(idx)
                                    .and_then(|p| p.path.parent())
                                    .and_then(|p| p.file_name())
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("");
                                let relevant: Vec<_> = if project_dir.is_empty() {
                                    preselected.iter().collect()
                                } else {
                                    preselected.iter()
                                        .filter(|ct| ct.file_path.replace('/', "\\").contains(project_dir)
                                            || ct.file_path.contains(project_dir))
                                        .collect()
                                };
                                if relevant.is_empty() {
                                    vec![]
                                } else {
                                    build_synthetic_classes(&relevant)
                                }
                            }
                        } else {
                            classes
                        };
                        
                        // Add all class names to collapsed set (start collapsed)
                        // Get project name first for the collapse key
                        let project_name = state.projects.get(idx)
                            .map(|p| p.name.clone())
                            .unwrap_or_default();
                        for class in &classes {
                            let collapse_key = AppState::collapse_key(&project_name, &class.full_name);
                            state.collapsed_classes.insert(collapse_key);
                        }
                        if let Some(project) = state.projects.get_mut(idx) {
                            project.classes = classes;
                        }
                        state.invalidate_test_items();
                    }
                    DiscoveryEvent::ProjectError(idx, error) => {
                        // Log the error to the output pane (first 3 lines for brevity)
                        if let Some(project) = state.projects.get(idx) {
                            let error_preview: String = error
                                .lines()
                                .take(3)
                                .collect::<Vec<_>>()
                                .join("\n  ");
                            state.append_output(&format!(
                                "\n[Discovery] {} failed:\n  {}\n",
                                project.name,
                                error_preview
                            ));
                        }
                    }
                    DiscoveryEvent::Complete => {
                        state.discovering = false;
                        state.status = "Ready".to_string();
                        state.append_output(&format!("\n{}", random_ready_phrase()));
                        
                        // Auto-select all tests in PR mode (they're already filtered)
                        if filter_to_preselected {
                            let mut count = 0;
                            for project in &state.projects {
                                for class in &project.classes {
                                    for test in &class.tests {
                                        state.selected_tests.insert(test.full_name.clone());
                                        count += 1;
                                    }
                                }
                            }
                            if count > 0 {
                                state.append_output(&format!("\n[PR] Loaded {} changed test(s).", count));
                                // Expand all classes to show selected tests
                                state.collapsed_classes.clear();
                                state.invalidate_test_items();
                            }
                        }
                    }
                }
            }
        }

        // Check for file changes in watch mode
        if state.watch_mode {
            if let Some(ref watcher) = file_watcher {
                if watcher.try_recv() && executor_rx.is_none() && !state.discovering {
                    state.dirty = true;
                    state.append_output("\n[Watch] File change detected, running tests...\n");
                    run_tests(&mut state, &mut executor_rx);
                }
            }
        }

        // Check for executor events
        if let Some(ref rx) = executor_rx {
            while let Ok(event) = rx.try_recv() {
                state.dirty = true;
                match event {
                    ExecutorEvent::OutputLine(line) => {
                        let trimmed = line.trim();
                        if trimmed.starts_with("Passed") || trimmed.starts_with("Failed") {
                            // Increment progress instead of appending
                            if let Some((completed, _)) = &mut state.test_progress {
                                *completed += 1;
                            }
                        }
                        // Ignore other dotnet output lines
                    }
                    ExecutorEvent::BuildCompleted(success) => {
                        if success {
                            state.append_output("\nBuild succeeded.");
                        } else {
                            state.append_output("\nBuild FAILED");
                        }
                        state.status = "Ready".to_string();
                        executor_rx = None;
                        break;
                    }
                    ExecutorEvent::Completed(results) => {
                        // Track failed tests and count results
                        state.last_failed.clear();
                        let mut passed = 0;
                        let mut failed = 0;
                        let mut skipped = 0;
                        for result in &results {
                            match result.outcome {
                                TestOutcome::Passed => passed += 1,
                                TestOutcome::Failed => {
                                    failed += 1;
                                    state.last_failed.insert(result.test_name.clone());
                                }
                                TestOutcome::Skipped => skipped += 1,
                            }
                        }

                        apply_results(&mut state, &results);
                        // Reset any tests still stuck in RUNNING (no TRX result matched)
                        reset_unmatched_running_tests(&mut state);
                        state.invalidate_test_items();

                        // Show summary
                        let total = passed + failed + skipped;
                        let mut summary = format!("\n{} tests run.", total);
                        if passed > 0 {
                            summary.push_str(&format!(" {}/{} passed.", passed, total));
                        }
                        if failed > 0 {
                            summary.push_str(&format!(" {} failed.", failed));
                        }
                        if skipped > 0 {
                            summary.push_str(&format!(" {} skipped.", skipped));
                        }
                        state.append_output(&summary);

                        state.test_progress = None;
                        state.status = "Ready".to_string();
                        executor_rx = None;
                        break;
                    }
                    ExecutorEvent::Error(e) => {
                        state.append_output(&format!("\nError: {}", e));
                        state.status = "Ready".to_string();
                        executor_rx = None;
                        break;
                    }
                }
            }
        }

        // Only redraw when state has changed
        if state.dirty {
            terminal.draw(|f| ui::draw(f, &mut state))?;
            state.dirty = false;
        }

        // Dynamic timeout: fast when active operations are running, slow when idle
        let poll_timeout = if state.discovering || executor_rx.is_some() {
            Duration::from_millis(33)
        } else {
            Duration::from_millis(250)
        };

        // Poll for keyboard input with timeout
        if event::poll(poll_timeout)? {
            match event::read()? {
                Event::Resize(_, _) => {
                    state.dirty = true;
                }
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    state.dirty = true;

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
                        KeyCode::Left => {
                            move_to_prev_group(&mut state);
                        }
                        KeyCode::Right => {
                            move_to_next_group(&mut state);
                        }
                        KeyCode::Tab => {
                            state.active_pane = match state.active_pane {
                                Pane::Projects => Pane::Tests,
                                Pane::Tests => Pane::Output,
                                Pane::Output => Pane::TestResult,
                                Pane::TestResult => Pane::Projects,
                            };
                        }
                        KeyCode::BackTab => {
                            state.active_pane = match state.active_pane {
                                Pane::Projects => Pane::TestResult,
                                Pane::Tests => Pane::Projects,
                                Pane::Output => Pane::Tests,
                                Pane::TestResult => Pane::Output,
                            };
                        }
                        KeyCode::Char(' ') => {
                            if state.active_pane == Pane::Tests {
                                toggle_space_action(&mut state);
                            }
                        }
                        KeyCode::Char('x') => {
                            state.clear_output();
                            state.test_progress = None;
                        }
                        KeyCode::Char('c') => {
                            // Expand/collapse all classes in current project
                            if let Some(idx) = state.project_state.selected() {
                                if let Some(project) = state.projects.get(idx) {
                                    let project_name = project.name.clone();
                                    let class_full_names: Vec<String> = project.classes.iter()
                                        .map(|c| c.full_name.clone())
                                        .collect();
                                    state.toggle_expand_collapse_all(&project_name, &class_full_names);
                                }
                            }
                        }
                        KeyCode::Char('C') => {
                            // Clear test selections
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
                                        state.append_output("\n[Watch] Watch mode enabled\n");
                                    }
                                    Err(e) => {
                                        state.watch_mode = false;
                                        state.append_output(&format!(
                                            "\n[Watch] Failed to enable watch mode: {}\n",
                                            e
                                        ));
                                    }
                                }
                            } else {
                                file_watcher = None;
                                state.append_output("\n[Watch] Watch mode disabled\n");
                            }
                        }
                        KeyCode::Char('r') => {
                            if executor_rx.is_none() && !state.discovering {
                                // If tests are multi-selected, run those
                                if !state.selected_tests.is_empty() {
                                    run_tests(&mut state, &mut executor_rx);
                                    continue;
                                }

                                // If in Tests pane, check what's under cursor
                                if state.active_pane == Pane::Tests {
                                    // Check if a class is selected - run all tests in that class
                                    if let Some(class_tests) = get_selected_class_tests(&mut state) {
                                        run_class_tests(&mut state, &mut executor_rx, class_tests);
                                        continue;
                                    }

                                    // Check if a single test is selected - run just that test
                                    if let Some(test_name) = get_selected_single_test(&mut state) {
                                        run_class_tests(&mut state, &mut executor_rx, vec![test_name]);
                                        continue;
                                    }
                                }

                                // Fallback: run all tests in project
                                run_tests(&mut state, &mut executor_rx);
                            }
                        }
                        KeyCode::Char('R') => {
                            // Shift+R: always run all tests in the project
                            if executor_rx.is_none() && !state.discovering {
                                run_tests(&mut state, &mut executor_rx);
                            }
                        }
                        KeyCode::Char('b') => {
                            if executor_rx.is_none() && !state.discovering {
                                build_project(&mut state, &mut executor_rx);
                            }
                        }
                        KeyCode::Char('a') => {
                            if executor_rx.is_none() && !state.discovering && !state.last_failed.is_empty() {
                                run_failed_tests(&mut state, &mut executor_rx);
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
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
            let len = state.get_test_items().len();
            if len == 0 {
                return;
            }
            let current = state.test_state.selected().unwrap_or(0) as i32;
            let new = (current + delta).rem_euclid(len as i32) as usize;
            state.test_state.select(Some(new));
        }
        Pane::Output => {
            let line_count = state.output_newline_count as i32;
            let current = state.output_scroll as i32;
            let new = (current + delta).max(0).min(line_count.saturating_sub(1));
            state.output_scroll = new as u16;
        }
        Pane::TestResult => {
            // Scroll within the test result pane
            let current = state.test_result_scroll as i32;
            let new = (current + delta).max(0) as u16;
            state.test_result_scroll = new;
        }
    }
}

fn toggle_collapse(state: &mut AppState) {
    let selected = state.test_state.selected();
    let project_idx = state.project_state.selected();
    let project_name = project_idx
        .and_then(|idx| state.projects.get(idx))
        .map(|p| p.name.clone())
        .unwrap_or_default();

    let class_name = {
        let items = state.get_test_items();
        if let Some(selected) = selected {
            if let Some(TestListItem::Class(class_name)) = items.get(selected) {
                class_name.clone()
            } else {
                return;
            }
        } else {
            return;
        }
    };
    state.toggle_class_collapsed(&project_name, &class_name);
    state.invalidate_test_items();
}

fn toggle_space_action(state: &mut AppState) {
    let selected = state.test_state.selected();
    let action = {
        let items = state.get_test_items();
        if let Some(selected) = selected {
            items.get(selected).map(|item| match item {
                TestListItem::Class(_) => None, // will call toggle_collapse
                TestListItem::Test(test_name) => Some(test_name.clone()),
            })
        } else {
            None
        }
    };
    match action {
        Some(None) => toggle_collapse(state),
        Some(Some(test_name)) => state.toggle_test_selected(&test_name),
        None => {}
    }
}

fn run_tests(
    state: &mut AppState,
    executor_rx: &mut Option<std::sync::mpsc::Receiver<ExecutorEvent>>,
) {
    if let Some(idx) = state.project_state.selected() {
        // Get path before mutating
        let path = if let Some(project) = state.projects.get(idx) {
            project.path.clone()
        } else {
            return;
        };

        // Store which project we're running tests for
        state.running_project_idx = Some(idx);

        // Determine which tests to run:
        // 1. If tests are selected, run only selected tests
        // 2. Else if filter is active, run only filtered tests
        // 3. Else run all tests
        let (tests_to_run, total_tests) = if !state.selected_tests.is_empty() {
            mark_selected_tests_running(state);
            let tests: Vec<String> = state.selected_tests.iter().cloned().collect();
            let count = tests.len();
            (Some(tests), count)
        } else if !state.filter.is_empty() {
            let filtered = get_filtered_tests(state);
            let count = filtered.len();
            if count == 0 {
                state.append_output("\nNo tests match the current filter.\n");
                return;
            }
            mark_filtered_tests_running(state, &filtered);
            let filtered_vec: Vec<String> = filtered.into_iter().collect();
            (Some(filtered_vec), count)
        } else {
            mark_all_tests_running(state);
            let count = state.projects.get(idx).map(|p| p.test_count()).unwrap_or(0);
            (None, count)
        };

        state.output_auto_scroll = true;
        state.append_output("\n────────────────────────────\n");
        state.append_output("Running tests...");
        state.test_progress = Some((0, total_tests));
        state.status = "Running tests...".to_string();

        let executor = TestExecutor::new(&path);
        *executor_rx = Some(executor.run(tests_to_run));
    }
}

fn build_project(
    state: &mut AppState,
    executor_rx: &mut Option<std::sync::mpsc::Receiver<ExecutorEvent>>,
) {
    if let Some(idx) = state.project_state.selected() {
        if let Some(project) = state.projects.get(idx) {
            let path = project.path.clone();

            state.output_auto_scroll = true;
            state.append_output("\n────────────────────────────\n");
            state.append_output("Building...\n");
            state.status = "Building...".to_string();

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

            // Store which project we're running tests for
            state.running_project_idx = Some(idx);

            // Mark failed tests as running
            let failed_count = state.last_failed.len();
            // Precompute suffixes for O(1) lookup
            let failed_suffixes: HashSet<String> = state.last_failed.iter()
                .filter_map(|f| f.rsplit_once('.').map(|(_, name)| name.to_string()))
                .collect();
            if let Some(project) = state.projects.get_mut(idx) {
                for class in &mut project.classes {
                    for test in &mut class.tests {
                        if state.last_failed.contains(&test.full_name)
                            || failed_suffixes.contains(&test.full_name)
                        {
                            test.status = TestStatus::Running;
                        }
                    }
                }
            }

            state.output_auto_scroll = true;
            state.append_output("\n────────────────────────────\n");
            state.append_output(&format!("Re-running {} failed...\n", failed_count));
            state.test_progress = Some((0, failed_count));
            state.status = "Running tests...".to_string();

            let executor = TestExecutor::new(&path);
            let filter: Vec<String> = state.last_failed.iter().cloned().collect();
            *executor_rx = Some(executor.run(Some(filter)));
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

fn get_filtered_tests(state: &AppState) -> HashSet<String> {
    let filter_lower = state.filter.to_lowercase();
    let mut tests = HashSet::new();

    if let Some(idx) = state.project_state.selected() {
        if let Some(project) = state.projects.get(idx) {
            for class in &project.classes {
                for test in &class.tests {
                    if test.name_lower.contains(&filter_lower) {
                        tests.insert(test.full_name.clone());
                    }
                }
            }
        }
    }

    tests
}

fn mark_filtered_tests_running(state: &mut AppState, filtered_tests: &HashSet<String>) {
    if let Some(idx) = state.project_state.selected() {
        if let Some(project) = state.projects.get_mut(idx) {
            for class in &mut project.classes {
                for test in &mut class.tests {
                    if filtered_tests.contains(&test.full_name) {
                        test.status = TestStatus::Running;
                    }
                }
            }
        }
    }
}

fn apply_results(state: &mut AppState, results: &[crate::parser::TestResult]) {
    use std::collections::HashMap;

    // Use running_project_idx to update the correct project, not the currently selected one
    let idx = state.running_project_idx.or(state.project_state.selected());
    if let Some(idx) = idx {
        if let Some(project) = state.projects.get_mut(idx) {
            // Build index: map from test_name -> Vec<(index, &result)>
            // Also map from suffix (last segment after '.') -> Vec<(index, &result)>
            let mut by_full_name: HashMap<&str, Vec<(usize, &crate::parser::TestResult)>> = HashMap::new();
            let mut by_suffix: HashMap<&str, Vec<(usize, &crate::parser::TestResult)>> = HashMap::new();
            let mut by_bare_name: HashMap<&str, Vec<(usize, &crate::parser::TestResult)>> = HashMap::new();

            for (i, r) in results.iter().enumerate() {
                by_full_name.entry(r.test_name.as_str()).or_default().push((i, r));
                // Extract the part after the last '.' for suffix matching
                if let Some(pos) = r.test_name.rfind('.') {
                    let suffix = &r.test_name[pos + 1..];
                    by_suffix.entry(suffix).or_default().push((i, r));
                }
                by_bare_name.entry(r.test_name.as_str()).or_default().push((i, r));
            }

            let mut consumed = vec![false; results.len()];

            // Pass 1: precise full_name matching, then suffix matching
            for class in &mut project.classes {
                for test in &mut class.tests {
                    // Try exact match first
                    let matched = by_full_name.get(test.full_name.as_str())
                        .and_then(|entries| entries.iter().find(|(i, _)| !consumed[*i]))
                        .or_else(|| {
                            // Try suffix match: result ends with ".{test.full_name}"
                            by_suffix.get(test.full_name.as_str())
                                .and_then(|entries| entries.iter().find(|(i, _)| !consumed[*i]))
                        });

                    if let Some(&(i, result)) = matched {
                        consumed[i] = true;
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

            // Pass 1.5: endsWith matching for multi-segment names (e.g. "Class.Method" vs "Namespace.Class.Method")
            let suffix_needle = ".";
            for class in &mut project.classes {
                for test in &mut class.tests {
                    if test.status != TestStatus::Running {
                        continue;
                    }
                    let needle = format!("{}{}", suffix_needle, test.full_name);
                    let matched = results.iter().enumerate()
                        .find(|(i, r)| !consumed[*i] && (r.test_name.ends_with(&needle) || r.test_name == test.full_name));
                    if let Some((i, result)) = matched {
                        consumed[i] = true;
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

            // Pass 2: bare name fallback for remaining unmatched tests/results
            for class in &mut project.classes {
                for test in &mut class.tests {
                    if test.status == TestStatus::Running {
                        // Try test.name directly
                        let matched = by_bare_name.get(test.name.as_str())
                            .and_then(|entries| entries.iter().find(|(i, _)| !consumed[*i]))
                            .or_else(|| {
                                // Try bare method name (after last '.') for Class.Method style names
                                let method = test.name.rsplit('.').next().unwrap_or(&test.name);
                                by_bare_name.get(method)
                                    .and_then(|entries| entries.iter().find(|(i, _)| !consumed[*i]))
                            });

                        if let Some(&(i, result)) = matched {
                            consumed[i] = true;
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
    // Clear running project index after applying results
    state.running_project_idx = None;
}

/// Reset any tests still in RUNNING state back to NotRun (no TRX result found for them)
fn reset_unmatched_running_tests(state: &mut AppState) {
    for project in &mut state.projects {
        for class in &mut project.classes {
            for test in &mut class.tests {
                if test.status == TestStatus::Running {
                    test.status = TestStatus::NotRun;
                }
            }
        }
    }
}

/// Get the tests for the currently selected class, if a class is selected.
fn get_selected_class_tests(state: &mut AppState) -> Option<Vec<String>> {
    let selected_idx = state.test_state.selected()?;
    let class_full_name = {
        let items = state.get_test_items();
        if let Some(TestListItem::Class(name)) = items.get(selected_idx) {
            name.clone()
        } else {
            return None;
        }
    };

    let project_idx = state.project_state.selected()?;
    let project = state.projects.get(project_idx)?;
    for class in &project.classes {
        if class.full_name == class_full_name {
            let tests: Vec<String> = class.tests.iter().map(|t| t.full_name.clone()).collect();
            if !tests.is_empty() {
                return Some(tests);
            }
        }
    }

    None
}

/// Get the currently selected single test, if a test (not class) is selected.
fn get_selected_single_test(state: &mut AppState) -> Option<String> {
    let selected_idx = state.test_state.selected()?;
    let items = state.get_test_items();
    if let Some(TestListItem::Test(test_full_name)) = items.get(selected_idx) {
        Some(test_full_name.clone())
    } else {
        None
    }
}

/// Run tests for a specific class.
fn run_class_tests(
    state: &mut AppState,
    executor_rx: &mut Option<std::sync::mpsc::Receiver<ExecutorEvent>>,
    tests: Vec<String>,
) {
    if let Some(idx) = state.project_state.selected() {
        let path = if let Some(project) = state.projects.get(idx) {
            project.path.clone()
        } else {
            return;
        };

        // Store which project we're running tests for
        state.running_project_idx = Some(idx);

        let test_count = tests.len();
        let test_set: HashSet<String> = tests.iter().cloned().collect();

        // Mark these tests as running
        if let Some(project) = state.projects.get_mut(idx) {
            for class in &mut project.classes {
                for test in &mut class.tests {
                    if test_set.contains(&test.full_name) {
                        test.status = TestStatus::Running;
                    }
                }
            }
        }

        state.output_auto_scroll = true;
        state.append_output("\n────────────────────────────\n");
        state.append_output(&format!("Running {} test(s)...\n", test_count));
        state.test_progress = Some((0, test_count));
        state.status = "Running tests...".to_string();

        let executor = TestExecutor::new(&path);
        *executor_rx = Some(executor.run(Some(tests)));
    }
}

/// Navigate to the next test group (class)
fn move_to_next_group(state: &mut AppState) {
    if state.active_pane != Pane::Tests {
        return;
    }

    let current_idx = state.test_state.selected().unwrap_or(0);
    let items = state.get_test_items();
    if items.is_empty() {
        return;
    }

    // Find next class after current position
    for (i, item) in items.iter().enumerate().skip(current_idx + 1) {
        if matches!(item, TestListItem::Class(_)) {
            state.test_state.select(Some(i));
            return;
        }
    }

    // Wrap around to first class
    for (i, item) in items.iter().enumerate() {
        if matches!(item, TestListItem::Class(_)) {
            state.test_state.select(Some(i));
            return;
        }
    }
}

/// Navigate to the previous test group (class)
fn move_to_prev_group(state: &mut AppState) {
    if state.active_pane != Pane::Tests {
        return;
    }

    let current_idx = state.test_state.selected().unwrap_or(0);
    let items = state.get_test_items();
    if items.is_empty() {
        return;
    }

    // Find previous class before current position
    for i in (0..current_idx).rev() {
        if matches!(items.get(i), Some(TestListItem::Class(_))) {
            state.test_state.select(Some(i));
            return;
        }
    }

    // Wrap around to last class
    for i in (0..items.len()).rev() {
        if matches!(items.get(i), Some(TestListItem::Class(_))) {
            state.test_state.select(Some(i));
            return;
        }
    }
}

/// Filter test classes to only include tests matching the given changed tests
fn filter_classes_to_tests(classes: &[crate::model::TestClass], changed_tests: &[ChangedTest]) -> Vec<crate::model::TestClass> {
    let name_set: HashSet<&str> = changed_tests.iter().map(|t| t.method_name.as_str()).collect();
    classes
        .iter()
        .filter_map(|class| {
            let filtered_tests: Vec<_> = class.tests
                .iter()
                .filter(|test| {
                    name_set.contains(test.name.as_str())
                        || changed_tests.iter().any(|ct| test.name.contains(ct.method_name.as_str()) || ct.method_name.contains(&test.name))
                })
                .cloned()
                .collect();

            if filtered_tests.is_empty() {
                None
            } else {
                let mut new_class = crate::model::TestClass::new(
                    class.name.clone(),
                    class.namespace.clone(),
                );
                new_class.tests = filtered_tests;
                Some(new_class)
            }
        })
        .collect()
}

/// Build synthetic test classes from PR changed tests, grouped by class name
fn build_synthetic_classes(changed_tests: &[&ChangedTest]) -> Vec<crate::model::TestClass> {
    use std::collections::HashMap;
    let mut class_map: HashMap<&str, Vec<&&ChangedTest>> = HashMap::new();
    for ct in changed_tests {
        class_map.entry(ct.class_name.as_str()).or_default().push(ct);
    }
    let mut classes = Vec::new();
    for (class_name, tests) in &class_map {
        let mut tc = crate::model::TestClass::new(
            class_name.to_string(),
            String::new(),
        );
        for ct in tests {
            let display_name = format!("{}.{}", class_name, ct.method_name);
            tc.tests.push(crate::model::Test::new(
                display_name.clone(),
                display_name,
            ));
        }
        classes.push(tc);
    }
    classes
}
