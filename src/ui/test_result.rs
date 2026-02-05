use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::model::{Test, TestStatus};
use crate::ui::theme::Theme;

pub struct TestResultPane<'a> {
    test: Option<&'a Test>,
    theme: &'a Theme,
    focused: bool,
    scroll: u16,
}

impl<'a> TestResultPane<'a> {
    pub fn new(test: Option<&'a Test>, theme: &'a Theme, focused: bool, scroll: u16) -> Self {
        Self {
            test,
            theme,
            focused,
            scroll,
        }
    }

    fn status_text(&self, status: &TestStatus) -> (&'static str, Style) {
        match status {
            TestStatus::NotRun => ("NOT RUN", Style::default().fg(self.theme.fg)),
            TestStatus::Running => (
                "RUNNING",
                Style::default()
                    .fg(self.theme.highlight)
                    .add_modifier(Modifier::BOLD),
            ),
            TestStatus::Passed => (
                "PASSED",
                Style::default()
                    .fg(self.theme.passed)
                    .add_modifier(Modifier::BOLD),
            ),
            TestStatus::Failed => (
                "FAILED",
                Style::default()
                    .fg(self.theme.failed)
                    .add_modifier(Modifier::BOLD),
            ),
            TestStatus::Skipped => (
                "SKIPPED",
                Style::default()
                    .fg(self.theme.skipped)
                    .add_modifier(Modifier::BOLD),
            ),
        }
    }
}

impl Widget for TestResultPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(self.theme.highlight)
        } else {
            Style::default().fg(self.theme.border)
        };

        let block = Block::default()
            .title(" Test Result ")
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner = block.inner(area);
        block.render(area, buf);

        match self.test {
            None => {
                let text = Paragraph::new("No test selected.")
                    .style(Style::default().fg(self.theme.fg));
                text.render(inner, buf);
            }
            Some(test) => {
                let mut lines: Vec<Line> = Vec::new();

                // Test name
                lines.push(Line::from(vec![
                    Span::styled("Test: ", Style::default().fg(self.theme.border)),
                    Span::styled(
                        &test.full_name,
                        Style::default()
                            .fg(self.theme.fg)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));

                // Status
                let (status_text, status_style) = self.status_text(&test.status);
                lines.push(Line::from(vec![
                    Span::styled("Status: ", Style::default().fg(self.theme.border)),
                    Span::styled(status_text, status_style),
                ]));

                // Duration
                if let Some(duration_ms) = test.duration_ms {
                    let duration_str = if duration_ms >= 1000 {
                        format!("{:.2}s", duration_ms as f64 / 1000.0)
                    } else {
                        format!("{}ms", duration_ms)
                    };
                    lines.push(Line::from(vec![
                        Span::styled("Duration: ", Style::default().fg(self.theme.border)),
                        Span::styled(duration_str, Style::default().fg(self.theme.fg)),
                    ]));
                }

                // Error message / stack trace
                if let Some(ref error) = test.error_message {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "Error:",
                        Style::default()
                            .fg(self.theme.failed)
                            .add_modifier(Modifier::BOLD),
                    )));

                    // Split error message into lines
                    for line in error.lines() {
                        lines.push(Line::from(Span::styled(
                            line.to_string(),
                            Style::default().fg(self.theme.fg),
                        )));
                    }
                }

                let text = Paragraph::new(lines)
                    .scroll((self.scroll, 0))
                    .wrap(Wrap { trim: false });
                text.render(inner, buf);
            }
        }
    }
}
