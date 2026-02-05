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

    /// Get aggregate status for a test class
    fn class_status(&self, class: &TestClass) -> TestStatus {
        let mut has_failed = false;
        let mut has_running = false;
        let mut has_passed = false;
        let mut all_not_run = true;

        for test in &class.tests {
            if !self.matches_filter(&test.name) {
                continue;
            }
            match test.status {
                TestStatus::Failed => {
                    has_failed = true;
                    all_not_run = false;
                }
                TestStatus::Running => {
                    has_running = true;
                    all_not_run = false;
                }
                TestStatus::Passed => {
                    has_passed = true;
                    all_not_run = false;
                }
                TestStatus::Skipped => {
                    all_not_run = false;
                }
                TestStatus::NotRun => {}
            }
        }

        if has_failed {
            TestStatus::Failed
        } else if has_running {
            TestStatus::Running
        } else if all_not_run {
            TestStatus::NotRun
        } else if has_passed {
            TestStatus::Passed
        } else {
            TestStatus::Skipped
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

            // Display name - use "Uncategorized" for empty class names
            let display_name = if class_full_name.is_empty() {
                "Uncategorized".to_string()
            } else {
                class_full_name.clone()
            };

            // Get aggregate status for the class
            let class_status = self.class_status(class);
            let (status_symbol, status_style) = self.status_symbol(&class_status);

            // Class header with collapse indicator and status
            let collapse_indicator = if is_collapsed { "+" } else { "-" };
            let test_count = class.tests.iter().filter(|t| self.matches_filter(&t.name)).count();
            let class_line = Line::from(vec![
                Span::styled(
                    format!("{} ", collapse_indicator),
                    Style::default().fg(self.theme.border),
                ),
                Span::styled(format!("{} ", status_symbol), status_style),
                Span::styled(
                    display_name,
                    Style::default()
                        .fg(self.theme.highlight)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({})", test_count),
                    Style::default().fg(self.theme.border),
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
                            format!("    {} ", select_marker),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Test;

    fn create_test_class(name: &str, namespace: &str, test_names: &[&str]) -> TestClass {
        let mut class = TestClass::new(name.to_string(), namespace.to_string());
        for test_name in test_names {
            class.tests.push(Test::new(
                test_name.to_string(),
                format!("{}.{}.{}", namespace, name, test_name),
            ));
        }
        class
    }

    // TestListItem tests
    #[test]
    fn test_list_item_class_variant() {
        let item = TestListItem::Class("NS.MyClass".to_string());
        match item {
            TestListItem::Class(name) => assert_eq!(name, "NS.MyClass"),
            _ => panic!("Expected Class variant"),
        }
    }

    #[test]
    fn test_list_item_test_variant() {
        let item = TestListItem::Test("NS.MyClass.TestMethod".to_string());
        match item {
            TestListItem::Test(name) => assert_eq!(name, "NS.MyClass.TestMethod"),
            _ => panic!("Expected Test variant"),
        }
    }

    #[test]
    fn test_list_item_clone() {
        let item = TestListItem::Class("MyClass".to_string());
        let cloned = item.clone();
        match cloned {
            TestListItem::Class(name) => assert_eq!(name, "MyClass"),
            _ => panic!("Expected Class variant"),
        }
    }

    // build_test_items tests - empty inputs
    #[test]
    fn test_build_test_items_empty_classes() {
        let collapsed = HashSet::new();
        let items = build_test_items(&[], &collapsed, "");
        assert!(items.is_empty());
    }

    // build_test_items tests - basic functionality
    #[test]
    fn test_build_test_items_single_class_single_test() {
        let classes = vec![create_test_class("MyClass", "NS", &["Test1"])];
        let collapsed = HashSet::new();

        let items = build_test_items(&classes, &collapsed, "");

        assert_eq!(items.len(), 2);
        match &items[0] {
            TestListItem::Class(name) => assert_eq!(name, "NS.MyClass"),
            _ => panic!("Expected Class"),
        }
        match &items[1] {
            TestListItem::Test(name) => assert_eq!(name, "NS.MyClass.Test1"),
            _ => panic!("Expected Test"),
        }
    }

    #[test]
    fn test_build_test_items_single_class_multiple_tests() {
        let classes = vec![create_test_class("MyClass", "NS", &["Test1", "Test2", "Test3"])];
        let collapsed = HashSet::new();

        let items = build_test_items(&classes, &collapsed, "");

        assert_eq!(items.len(), 4); // 1 class + 3 tests
        match &items[0] {
            TestListItem::Class(_) => (),
            _ => panic!("Expected Class"),
        }
        for i in 1..4 {
            match &items[i] {
                TestListItem::Test(_) => (),
                _ => panic!("Expected Test at index {}", i),
            }
        }
    }

    #[test]
    fn test_build_test_items_multiple_classes() {
        let classes = vec![
            create_test_class("ClassA", "NS", &["Test1"]),
            create_test_class("ClassB", "NS", &["Test1", "Test2"]),
        ];
        let collapsed = HashSet::new();

        let items = build_test_items(&classes, &collapsed, "");

        assert_eq!(items.len(), 5); // 2 classes + 3 tests total
    }

    // build_test_items tests - collapsed state
    #[test]
    fn test_build_test_items_collapsed_class() {
        let classes = vec![create_test_class("MyClass", "NS", &["Test1", "Test2"])];
        let mut collapsed = HashSet::new();
        collapsed.insert("NS.MyClass".to_string());

        let items = build_test_items(&classes, &collapsed, "");

        assert_eq!(items.len(), 1); // Only the class header, no tests
        match &items[0] {
            TestListItem::Class(name) => assert_eq!(name, "NS.MyClass"),
            _ => panic!("Expected Class"),
        }
    }

    #[test]
    fn test_build_test_items_mixed_collapsed() {
        let classes = vec![
            create_test_class("ClassA", "NS", &["Test1"]),
            create_test_class("ClassB", "NS", &["Test1", "Test2"]),
        ];
        let mut collapsed = HashSet::new();
        collapsed.insert("NS.ClassA".to_string()); // Only ClassA is collapsed

        let items = build_test_items(&classes, &collapsed, "");

        // ClassA (collapsed): 1 item
        // ClassB (expanded): 1 class + 2 tests = 3 items
        assert_eq!(items.len(), 4);
    }

    // build_test_items tests - filtering
    #[test]
    fn test_build_test_items_filter_matches_all() {
        let classes = vec![create_test_class("MyClass", "NS", &["Test1", "Test2"])];
        let collapsed = HashSet::new();

        let items = build_test_items(&classes, &collapsed, "Test");

        assert_eq!(items.len(), 3); // 1 class + 2 tests
    }

    #[test]
    fn test_build_test_items_filter_matches_some() {
        let classes = vec![create_test_class("MyClass", "NS", &["Test1", "Other2"])];
        let collapsed = HashSet::new();

        let items = build_test_items(&classes, &collapsed, "Test");

        assert_eq!(items.len(), 2); // 1 class + 1 matching test
    }

    #[test]
    fn test_build_test_items_filter_matches_none() {
        let classes = vec![create_test_class("MyClass", "NS", &["Test1", "Test2"])];
        let collapsed = HashSet::new();

        let items = build_test_items(&classes, &collapsed, "NonExistent");

        assert!(items.is_empty()); // Class is excluded because no tests match
    }

    #[test]
    fn test_build_test_items_filter_case_insensitive() {
        let classes = vec![create_test_class("MyClass", "NS", &["TestMethod"])];
        let collapsed = HashSet::new();

        // Filter with different case
        let items = build_test_items(&classes, &collapsed, "testmethod");
        assert_eq!(items.len(), 2);

        let items = build_test_items(&classes, &collapsed, "TESTMETHOD");
        assert_eq!(items.len(), 2);

        let items = build_test_items(&classes, &collapsed, "TeSt");
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_build_test_items_filter_partial_match() {
        let classes = vec![create_test_class("MyClass", "NS", &["TestCalculation", "TestValidation"])];
        let collapsed = HashSet::new();

        let items = build_test_items(&classes, &collapsed, "Calc");
        assert_eq!(items.len(), 2); // 1 class + 1 test (only TestCalculation matches)

        let items = build_test_items(&classes, &collapsed, "Test");
        assert_eq!(items.len(), 3); // 1 class + 2 tests (both match)
    }

    #[test]
    fn test_build_test_items_filter_with_collapsed_class() {
        let classes = vec![create_test_class("MyClass", "NS", &["Test1", "Test2"])];
        let mut collapsed = HashSet::new();
        collapsed.insert("NS.MyClass".to_string());

        // Even with filter, collapsed class shows only header
        let items = build_test_items(&classes, &collapsed, "Test");
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_build_test_items_filter_excludes_class_with_no_matches() {
        let classes = vec![
            create_test_class("ClassA", "NS", &["Foo1"]),
            create_test_class("ClassB", "NS", &["Test1"]),
        ];
        let collapsed = HashSet::new();

        let items = build_test_items(&classes, &collapsed, "Test");

        // ClassA has no matching tests, so it should be excluded
        assert_eq!(items.len(), 2); // Only ClassB and its test
        match &items[0] {
            TestListItem::Class(name) => assert_eq!(name, "NS.ClassB"),
            _ => panic!("Expected ClassB"),
        }
    }

    // build_test_items tests - edge cases
    #[test]
    fn test_build_test_items_empty_filter() {
        let classes = vec![create_test_class("MyClass", "NS", &["Test1"])];
        let collapsed = HashSet::new();

        let items = build_test_items(&classes, &collapsed, "");

        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_build_test_items_class_with_no_tests() {
        let classes = vec![TestClass::new("EmptyClass".to_string(), "NS".to_string())];
        let collapsed = HashSet::new();

        let items = build_test_items(&classes, &collapsed, "");

        // Class with no tests should still show up when no filter
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_build_test_items_class_with_no_tests_and_filter() {
        let classes = vec![TestClass::new("EmptyClass".to_string(), "NS".to_string())];
        let collapsed = HashSet::new();

        let items = build_test_items(&classes, &collapsed, "Test");

        // Class with no tests should be excluded when filter is active
        assert!(items.is_empty());
    }

    #[test]
    fn test_build_test_items_preserves_test_full_name() {
        let classes = vec![create_test_class("MyClass", "Deep.Nested.NS", &["TestMethod"])];
        let collapsed = HashSet::new();

        let items = build_test_items(&classes, &collapsed, "");

        match &items[1] {
            TestListItem::Test(name) => {
                assert_eq!(name, "Deep.Nested.NS.MyClass.TestMethod");
            }
            _ => panic!("Expected Test"),
        }
    }

    #[test]
    fn test_build_test_items_class_without_namespace() {
        let classes = vec![create_test_class("MyClass", "", &["Test1"])];
        let collapsed = HashSet::new();

        let items = build_test_items(&classes, &collapsed, "");

        match &items[0] {
            TestListItem::Class(name) => assert_eq!(name, "MyClass"),
            _ => panic!("Expected Class"),
        }
    }
}
