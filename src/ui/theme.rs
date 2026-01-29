use ratatui::style::Color;

pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub highlight: Color,
    pub border: Color,
    pub passed: Color,
    pub failed: Color,
    pub running: Color,
    pub skipped: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: Color::Black,
            fg: Color::Rgb(255, 191, 0),      // Amber
            highlight: Color::Rgb(255, 215, 0), // Gold
            border: Color::Rgb(139, 119, 42),  // Dark gold
            passed: Color::Green,
            failed: Color::Red,
            running: Color::Yellow,
            skipped: Color::DarkGray,
        }
    }
}
