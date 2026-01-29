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
}

impl<'a> TestList<'a> {
    pub fn new(classes: &'a [TestClass], theme: &'a Theme) -> Self {
        Self { classes, theme }
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
}

impl StatefulWidget for TestList<'_> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let mut items: Vec<ListItem> = Vec::new();

        for class in self.classes {
            // Class header
            let class_line = Line::from(vec![Span::styled(
                class.full_name(),
                Style::default()
                    .fg(self.theme.fg)
                    .add_modifier(Modifier::BOLD),
            )]);
            items.push(ListItem::new(class_line));

            // Tests under this class
            for test in &class.tests {
                let (symbol, style) = self.status_symbol(&test.status);
                let test_line = Line::from(vec![
                    Span::styled(format!("  {} ", symbol), style),
                    Span::styled(&test.name, Style::default().fg(self.theme.fg)),
                ]);
                items.push(ListItem::new(test_line));
            }
        }

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Tests")
                    .border_style(Style::default().fg(self.theme.border)),
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
