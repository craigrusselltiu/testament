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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    pub output_scroll: u16,
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
            output_scroll: 0,
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
        state.output_scroll,
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
            "b:build",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Test, TestClass};
    use std::path::PathBuf;

    fn create_test_project(name: &str, test_count: usize) -> TestProject {
        let mut project = TestProject::new(name.to_string(), PathBuf::from(format!("/{}.csproj", name)));
        let mut class = TestClass::new("TestClass".to_string(), "NS".to_string());
        for i in 0..test_count {
            class.tests.push(Test::new(format!("test{}", i), format!("NS.TestClass.test{}", i)));
        }
        project.classes.push(class);
        project
    }

    // Pane tests
    #[test]
    fn test_pane_equality() {
        assert_eq!(Pane::Projects, Pane::Projects);
        assert_eq!(Pane::Tests, Pane::Tests);
        assert_eq!(Pane::Output, Pane::Output);
    }

    #[test]
    fn test_pane_inequality() {
        assert_ne!(Pane::Projects, Pane::Tests);
        assert_ne!(Pane::Tests, Pane::Output);
        assert_ne!(Pane::Projects, Pane::Output);
    }

    #[test]
    fn test_pane_clone() {
        let pane = Pane::Tests;
        let cloned = pane;
        assert_eq!(pane, cloned);
    }

    #[test]
    fn test_pane_copy() {
        let pane = Pane::Output;
        let copied = pane;
        assert_eq!(pane, copied);
    }

    // AppState::new tests
    #[test]
    fn test_app_state_new_empty_projects() {
        let state = AppState::new(vec![]);

        assert!(state.projects.is_empty());
        assert!(state.project_state.selected().is_none());
        assert!(state.test_state.selected().is_none());
        assert!(state.output.is_empty());
        assert_eq!(state.active_pane, Pane::Projects);
        assert!(state.collapsed_classes.is_empty());
        assert!(state.selected_tests.is_empty());
        assert!(state.filter.is_empty());
        assert!(!state.filter_active);
        assert!(!state.watch_mode);
        assert!(state.last_failed.is_empty());
    }

    #[test]
    fn test_app_state_new_with_projects() {
        let projects = vec![
            create_test_project("Project1", 2),
            create_test_project("Project2", 3),
        ];
        let state = AppState::new(projects);

        assert_eq!(state.projects.len(), 2);
        assert_eq!(state.project_state.selected(), Some(0));
    }

    #[test]
    fn test_app_state_new_selects_first_project() {
        let projects = vec![create_test_project("Project1", 1)];
        let state = AppState::new(projects);

        assert_eq!(state.project_state.selected(), Some(0));
    }

    // selected_project tests
    #[test]
    fn test_selected_project_with_selection() {
        let projects = vec![
            create_test_project("Project1", 1),
            create_test_project("Project2", 2),
        ];
        let state = AppState::new(projects);

        let selected = state.selected_project();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().name, "Project1");
    }

    #[test]
    fn test_selected_project_empty_projects() {
        let state = AppState::new(vec![]);

        assert!(state.selected_project().is_none());
    }

    #[test]
    fn test_selected_project_after_manual_selection() {
        let projects = vec![
            create_test_project("Project1", 1),
            create_test_project("Project2", 2),
        ];
        let mut state = AppState::new(projects);

        state.project_state.select(Some(1));
        let selected = state.selected_project();
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().name, "Project2");
    }

    // toggle_class_collapsed tests
    #[test]
    fn test_toggle_class_collapsed_add() {
        let mut state = AppState::new(vec![]);

        state.toggle_class_collapsed("NS.MyClass");
        assert!(state.collapsed_classes.contains("NS.MyClass"));
    }

    #[test]
    fn test_toggle_class_collapsed_remove() {
        let mut state = AppState::new(vec![]);

        state.toggle_class_collapsed("NS.MyClass");
        assert!(state.collapsed_classes.contains("NS.MyClass"));

        state.toggle_class_collapsed("NS.MyClass");
        assert!(!state.collapsed_classes.contains("NS.MyClass"));
    }

    #[test]
    fn test_toggle_class_collapsed_multiple_classes() {
        let mut state = AppState::new(vec![]);

        state.toggle_class_collapsed("Class1");
        state.toggle_class_collapsed("Class2");
        state.toggle_class_collapsed("Class3");

        assert!(state.collapsed_classes.contains("Class1"));
        assert!(state.collapsed_classes.contains("Class2"));
        assert!(state.collapsed_classes.contains("Class3"));

        state.toggle_class_collapsed("Class2");
        assert!(state.collapsed_classes.contains("Class1"));
        assert!(!state.collapsed_classes.contains("Class2"));
        assert!(state.collapsed_classes.contains("Class3"));
    }

    #[test]
    fn test_toggle_class_collapsed_empty_string() {
        let mut state = AppState::new(vec![]);

        state.toggle_class_collapsed("");
        assert!(state.collapsed_classes.contains(""));
    }

    // toggle_test_selected tests
    #[test]
    fn test_toggle_test_selected_add() {
        let mut state = AppState::new(vec![]);

        state.toggle_test_selected("NS.Class.Test1");
        assert!(state.selected_tests.contains("NS.Class.Test1"));
    }

    #[test]
    fn test_toggle_test_selected_remove() {
        let mut state = AppState::new(vec![]);

        state.toggle_test_selected("NS.Class.Test1");
        assert!(state.selected_tests.contains("NS.Class.Test1"));

        state.toggle_test_selected("NS.Class.Test1");
        assert!(!state.selected_tests.contains("NS.Class.Test1"));
    }

    #[test]
    fn test_toggle_test_selected_multiple_tests() {
        let mut state = AppState::new(vec![]);

        state.toggle_test_selected("Test1");
        state.toggle_test_selected("Test2");
        state.toggle_test_selected("Test3");

        assert_eq!(state.selected_tests.len(), 3);
        assert!(state.selected_tests.contains("Test1"));
        assert!(state.selected_tests.contains("Test2"));
        assert!(state.selected_tests.contains("Test3"));
    }

    // clear_selection tests
    #[test]
    fn test_clear_selection_empty() {
        let mut state = AppState::new(vec![]);

        state.clear_selection();
        assert!(state.selected_tests.is_empty());
    }

    #[test]
    fn test_clear_selection_with_selections() {
        let mut state = AppState::new(vec![]);

        state.toggle_test_selected("Test1");
        state.toggle_test_selected("Test2");
        state.toggle_test_selected("Test3");
        assert_eq!(state.selected_tests.len(), 3);

        state.clear_selection();
        assert!(state.selected_tests.is_empty());
    }

    // State field tests
    #[test]
    fn test_app_state_output_modification() {
        let mut state = AppState::new(vec![]);

        state.output = "Test output".to_string();
        assert_eq!(state.output, "Test output");

        state.output.push_str("\nMore output");
        assert_eq!(state.output, "Test output\nMore output");
    }

    #[test]
    fn test_app_state_filter_modification() {
        let mut state = AppState::new(vec![]);

        state.filter = "search term".to_string();
        assert_eq!(state.filter, "search term");
    }

    #[test]
    fn test_app_state_filter_active_toggle() {
        let mut state = AppState::new(vec![]);

        assert!(!state.filter_active);
        state.filter_active = true;
        assert!(state.filter_active);
    }

    #[test]
    fn test_app_state_watch_mode_toggle() {
        let mut state = AppState::new(vec![]);

        assert!(!state.watch_mode);
        state.watch_mode = true;
        assert!(state.watch_mode);
    }

    #[test]
    fn test_app_state_active_pane_change() {
        let mut state = AppState::new(vec![]);

        assert_eq!(state.active_pane, Pane::Projects);

        state.active_pane = Pane::Tests;
        assert_eq!(state.active_pane, Pane::Tests);

        state.active_pane = Pane::Output;
        assert_eq!(state.active_pane, Pane::Output);

        state.active_pane = Pane::Projects;
        assert_eq!(state.active_pane, Pane::Projects);
    }

    #[test]
    fn test_app_state_last_failed_modification() {
        let mut state = AppState::new(vec![]);

        state.last_failed.insert("FailedTest1".to_string());
        state.last_failed.insert("FailedTest2".to_string());

        assert_eq!(state.last_failed.len(), 2);
        assert!(state.last_failed.contains("FailedTest1"));
        assert!(state.last_failed.contains("FailedTest2"));
    }

    #[test]
    fn test_app_state_project_access() {
        let projects = vec![
            create_test_project("Project1", 5),
            create_test_project("Project2", 3),
        ];
        let state = AppState::new(projects);

        assert_eq!(state.projects[0].name, "Project1");
        assert_eq!(state.projects[0].test_count(), 5);
        assert_eq!(state.projects[1].name, "Project2");
        assert_eq!(state.projects[1].test_count(), 3);
    }
}
