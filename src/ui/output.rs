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

        let block = Block::default()
            .borders(Borders::ALL)
            .title("Output")
            .border_style(border_style);

        let inner = block.inner(area);
        block.render(area, buf);

        if let Some((completed, total)) = self.progress {
            // Render content in upper area, progress bar in bottom line
            if inner.height > 1 {
                let content_area = Rect { height: inner.height - 1, ..inner };
                let bar_area = Rect {
                    y: inner.y + inner.height - 1,
                    height: 1,
                    ..inner
                };

                let paragraph = Paragraph::new(self.content)
                    .style(Style::default().fg(self.theme.fg))
                    .wrap(Wrap { trim: false })
                    .scroll((self.scroll, 0));
                paragraph.render(content_area, buf);

                let width = 20usize;
                let filled = if total > 0 { (completed * width) / total } else { 0 };
                let bar_text = format!(
                    " [{}{}] {}/{}",
                    "\u{2588}".repeat(filled),
                    "\u{2591}".repeat(width - filled),
                    completed,
                    total
                );
                let bar_widget = Paragraph::new(bar_text)
                    .style(Style::default().fg(self.theme.highlight));
                bar_widget.render(bar_area, buf);
            }
        } else {
            let paragraph = Paragraph::new(self.content)
                .style(Style::default().fg(self.theme.fg))
                .wrap(Wrap { trim: false })
                .scroll((self.scroll, 0));
            paragraph.render(inner, buf);
        }
    }
}
