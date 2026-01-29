use std::collections::HashSet;

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    widgets::{Block, Borders, ListState, Paragraph},
    Frame,
};

use crate::model::TestProject;
use crate::ui::output::OutputPane;
use crate::ui::projects::ProjectList;
use crate::ui::tests::TestList;
use crate::ui::theme::Theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Projects,
    Tests,
    Output,
}

pub struct AppState {
    pub projects: Vec<TestProject>,
    pub project_state: ListState,
    pub test_state: ListState,
    pub output: String,
    pub theme: Theme,
    pub active_pane: Pane,
    pub collapsed_classes: HashSet<String>,
    pub selected_tests: HashSet<String>,
    pub filter: String,
    pub filter_active: bool,
    pub watch_mode: bool,
    pub last_failed: HashSet<String>,
}

impl AppState {
    pub fn new(projects: Vec<TestProject>) -> Self {
        let mut project_state = ListState::default();
        if !projects.is_empty() {
            project_state.select(Some(0));
        }
        Self {
            projects,
            project_state,
            test_state: ListState::default(),
            output: String::new(),
            theme: Theme::default(),
            active_pane: Pane::Projects,
            collapsed_classes: HashSet::new(),
            selected_tests: HashSet::new(),
            filter: String::new(),
            filter_active: false,
            watch_mode: false,
            last_failed: HashSet::new(),
        }
    }

    pub fn selected_project(&self) -> Option<&TestProject> {
        self.project_state
            .selected()
            .and_then(|i| self.projects.get(i))
    }

    pub fn toggle_class_collapsed(&mut self, class_name: &str) {
        if self.collapsed_classes.contains(class_name) {
            self.collapsed_classes.remove(class_name);
        } else {
            self.collapsed_classes.insert(class_name.to_string());
        }
    }

    pub fn toggle_test_selected(&mut self, test_name: &str) {
        if self.selected_tests.contains(test_name) {
            self.selected_tests.remove(test_name);
        } else {
            self.selected_tests.insert(test_name.to_string());
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_tests.clear();
    }
}

pub fn draw(frame: &mut Frame, state: &mut AppState) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(frame.area());

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(40),
            Constraint::Percentage(40),
        ])
        .split(main_chunks[0]);

    // Left pane: Projects
    let project_list = ProjectList::new(
        &state.projects,
        &state.theme,
        state.active_pane == Pane::Projects,
    );
    frame.render_stateful_widget(project_list, chunks[0], &mut state.project_state);

    // Middle pane: Tests
    let selected_idx = state.project_state.selected();
    let classes: &[_] = selected_idx
        .and_then(|i| state.projects.get(i))
        .map(|p| p.classes.as_slice())
        .unwrap_or(&[]);
    let test_list = TestList::new(
        classes,
        &state.theme,
        state.active_pane == Pane::Tests,
        &state.collapsed_classes,
        &state.selected_tests,
        &state.filter,
    );
    frame.render_stateful_widget(test_list, chunks[1], &mut state.test_state);

    // Right pane: Output
    let output_pane = OutputPane::new(
        &state.output,
        &state.theme,
        state.active_pane == Pane::Output,
    );
    frame.render_widget(output_pane, chunks[2]);

    // Status bar
    let watch_indicator = if state.watch_mode { "[WATCH] " } else { "" };
    let status = if state.filter_active {
        format!("{}Filter: {}_", watch_indicator, state.filter)
    } else {
        let selected_count = state.selected_tests.len();
        let failed_count = state.last_failed.len();
        let mut parts = vec![
            "q:quit",
            "r:run",
            "w:watch",
            "Tab:switch",
        ];
        if failed_count > 0 {
            parts.push("a:run-failed");
        }
        parts.extend(["Space:select", "/:filter"]);

        let suffix = if selected_count > 0 {
            format!(" | {} selected", selected_count)
        } else if failed_count > 0 {
            format!(" | {} failed", failed_count)
        } else {
            String::new()
        };

        format!("{}{}{}", watch_indicator, parts.join("  "), suffix)
    };
    let status_bar = Paragraph::new(status)
        .style(Style::default().fg(state.theme.fg).add_modifier(Modifier::DIM));
    frame.render_widget(status_bar, main_chunks[1]);
}
