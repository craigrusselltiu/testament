use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, StatefulWidget},
};

use crate::model::TestProject;
use crate::ui::theme::Theme;

pub struct ProjectList<'a> {
    projects: &'a [TestProject],
    theme: &'a Theme,
}

impl<'a> ProjectList<'a> {
    pub fn new(projects: &'a [TestProject], theme: &'a Theme) -> Self {
        Self { projects, theme }
    }
}

impl StatefulWidget for ProjectList<'_> {
    type State = ListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let items: Vec<ListItem> = self
            .projects
            .iter()
            .map(|p| {
                let line = Line::from(format!("{} ({})", p.name, p.test_count()));
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Projects")
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
