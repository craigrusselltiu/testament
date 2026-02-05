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
    focused: bool,
    scroll: u16,
    progress: Option<(usize, usize)>,
}

impl<'a> OutputPane<'a> {
    pub fn new(
        content: &'a str,
        theme: &'a Theme,
        focused: bool,
        scroll: u16,
        progress: Option<(usize, usize)>,
    ) -> Self {
        Self { content, theme, focused, scroll, progress }
    }
}

impl Widget for OutputPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(self.theme.highlight)
        } else {
            Style::default().fg(self.theme.border)
        };

        let display_content = if let Some((completed, total)) = self.progress {
            let width = 20;
            let filled = if total > 0 { (completed * width) / total } else { 0 };
            let bar = format!(
                " [{}{}] {}/{}",
                "\u{2588}".repeat(filled),
                "\u{2591}".repeat(width - filled),
                completed,
                total
            );
            format!("{}{}", self.content, bar)
        } else {
            self.content.to_string()
        };

        let paragraph = Paragraph::new(display_content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Output")
                    .border_style(border_style),
            )
            .style(Style::default().fg(self.theme.fg))
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));

        Widget::render(paragraph, area, buf);
    }
}
