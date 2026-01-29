use std::io;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::model::TestProject;
use crate::ui::{self, layout::AppState};

pub fn run(projects: Vec<TestProject>) -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new(projects);
    state.output = "Press 'r' to run tests, 'q' to quit".to_string();

    // Main loop
    loop {
        terminal.draw(|f| ui::draw(f, &mut state))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Char('j') | KeyCode::Down => {
                    move_selection(&mut state, 1);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    move_selection(&mut state, -1);
                }
                KeyCode::Tab => {
                    // TODO: Switch focus between panes
                }
                KeyCode::Char('r') => {
                    state.output = "Running tests... (not implemented yet)".to_string();
                }
                _ => {}
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn move_selection(state: &mut AppState, delta: i32) {
    let len = state.projects.len();
    if len == 0 {
        return;
    }
    let current = state.project_state.selected().unwrap_or(0) as i32;
    let new = (current + delta).rem_euclid(len as i32) as usize;
    state.project_state.select(Some(new));
}
