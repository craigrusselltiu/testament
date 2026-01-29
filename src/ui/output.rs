use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::ui::theme::Theme;

pub struct OutputPane<'a> {
    content: &'a str,
    theme: &'a Theme,
}

impl<'a> OutputPane<'a> {
    pub fn new(content: &'a str, theme: &'a Theme) -> Self {
        Self { content, theme }
    }
}

impl Widget for OutputPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let paragraph = Paragraph::new(self.content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Output")
                    .border_style(Style::default().fg(self.theme.border)),
            )
            .style(Style::default().fg(self.theme.fg))
            .wrap(Wrap { trim: false });

        Widget::render(paragraph, area, buf);
    }
}
