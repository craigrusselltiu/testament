use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::ListState,
    Frame,
};

use crate::model::TestProject;
use crate::ui::output::OutputPane;
use crate::ui::projects::ProjectList;
use crate::ui::tests::TestList;
use crate::ui::theme::Theme;

pub struct AppState {
    pub projects: Vec<TestProject>,
    pub project_state: ListState,
    pub test_state: ListState,
    pub output: String,
    pub theme: Theme,
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
        }
    }

    pub fn selected_project(&self) -> Option<&TestProject> {
        self.project_state
            .selected()
            .and_then(|i| self.projects.get(i))
    }
}

pub fn draw(frame: &mut Frame, state: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(40),
            Constraint::Percentage(40),
        ])
        .split(frame.area());

    // Left pane: Projects
    let project_list = ProjectList::new(&state.projects, &state.theme);
    frame.render_stateful_widget(project_list, chunks[0], &mut state.project_state);

    // Middle pane: Tests
    let selected_idx = state.project_state.selected();
    let classes: &[_] = selected_idx
        .and_then(|i| state.projects.get(i))
        .map(|p| p.classes.as_slice())
        .unwrap_or(&[]);
    let test_list = TestList::new(classes, &state.theme);
    frame.render_stateful_widget(test_list, chunks[1], &mut state.test_state);

    // Right pane: Output
    let output_pane = OutputPane::new(&state.output, &state.theme);
    frame.render_widget(output_pane, chunks[2]);
}
