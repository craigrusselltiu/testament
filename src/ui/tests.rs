use std::collections::HashSet;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget},
};

use crate::model::{TestClass, TestStatus};
use crate::ui::theme::Theme;

pub struct TestList<'a> {
    classes: &'a [TestClass],
    theme: &'a Theme,
    focused: bool,
    collapsed: &'a HashSet<String>,
    selected: &'a HashSet<String>,
    filter: &'a str,
}

impl<'a> TestList<'a> {
    pub fn new(
        classes: &'a [TestClass],
        theme: &'a Theme,
        focused: bool,
        collapsed: &'a HashSet<String>,
        selected: &'a HashSet<String>,
        filter: &'a str,
    ) -> Self {
        Self {
            classes,
            theme,
            focused,
            collapsed,
            selected,
            filter,
        }
    }

    fn status_symbol(&self, status: &TestStatus) -> (&str, Style) {
        match status {
            TestStatus::NotRun => (" ", Style::default().fg(self.theme.fg)),
            TestStatus::Running => ("*", Style::default().fg(self.theme.running)),
            TestStatus::Passed => ("+", Style::default().fg(self.theme.passed)),
            TestStatus::Failed => ("x", Style::default().fg(self.theme.failed)),
            TestStatus::Skipped => ("-", Style::default().fg(self.theme.skipped)),
        }
    }

    fn matches_filter(&self, name: &str) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        name.to_lowercase().contains(&self.filter.to_lowercase())
    }
}

impl StatefulWidget for TestList<'_> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let mut items: Vec<ListItem> = Vec::new();

        for class in self.classes {
            let class_full_name = class.full_name();
            let is_collapsed = self.collapsed.contains(&class_full_name);

            // Check if any tests in this class match the filter
            let has_matching_tests = class.tests.iter().any(|t| self.matches_filter(&t.name));
            if !has_matching_tests && !self.filter.is_empty() {
                continue;
            }

            // Class header with collapse indicator
            let collapse_indicator = if is_collapsed { "+" } else { "-" };
            let class_line = Line::from(vec![
                Span::styled(
                    format!("{} ", collapse_indicator),
                    Style::default().fg(self.theme.fg),
                ),
                Span::styled(
                    class_full_name,
                    Style::default()
                        .fg(self.theme.fg)
                        .add_modifier(Modifier::BOLD),
                ),
            ]);
            items.push(ListItem::new(class_line));

            // Tests under this class (if not collapsed)
            if !is_collapsed {
                for test in &class.tests {
                    if !self.matches_filter(&test.name) {
                        continue;
                    }

                    let (symbol, style) = self.status_symbol(&test.status);
                    let is_selected = self.selected.contains(&test.full_name);
                    let select_marker = if is_selected { "[x]" } else { "[ ]" };

                    let test_line = Line::from(vec![
                        Span::styled(
                            format!("  {} ", select_marker),
                            if is_selected {
                                Style::default().fg(self.theme.highlight)
                            } else {
                                Style::default().fg(self.theme.fg)
                            },
                        ),
                        Span::styled(format!("{} ", symbol), style),
                        Span::styled(&test.name, Style::default().fg(self.theme.fg)),
                    ]);
                    items.push(ListItem::new(test_line));
                }
            }
        }

        let border_style = if self.focused {
            Style::default().fg(self.theme.highlight)
        } else {
            Style::default().fg(self.theme.border)
        };

        let title = if self.filter.is_empty() {
            "Tests".to_string()
        } else {
            format!("Tests (filter: {})", self.filter)
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(border_style),
            )
            .style(Style::default().fg(self.theme.fg))
            .highlight_style(
                Style::default()
                    .fg(self.theme.highlight)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        StatefulWidget::render(list, area, buf, state);
    }
}

/// Represents an item in the flattened test list for navigation
#[derive(Clone)]
pub enum TestListItem {
    Class(String),
    Test(String),
}

/// Build a flattened list of items for navigation purposes
pub fn build_test_items(
    classes: &[TestClass],
    collapsed: &HashSet<String>,
    filter: &str,
) -> Vec<TestListItem> {
    let mut items = Vec::new();
    let filter_lower = filter.to_lowercase();

    for class in classes {
        let class_full_name = class.full_name();
        let is_collapsed = collapsed.contains(&class_full_name);

        let has_matching_tests = class.tests.iter().any(|t| {
            filter.is_empty() || t.name.to_lowercase().contains(&filter_lower)
        });

        if !has_matching_tests && !filter.is_empty() {
            continue;
        }

        items.push(TestListItem::Class(class_full_name.clone()));

        if !is_collapsed {
            for test in &class.tests {
                if filter.is_empty() || test.name.to_lowercase().contains(&filter_lower) {
                    items.push(TestListItem::Test(test.full_name.clone()));
                }
            }
        }
    }

    items
}
