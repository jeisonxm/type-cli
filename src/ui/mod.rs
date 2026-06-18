//! The rendering layer (stealth): a pure function of application state into a ratatui `Frame`. It
//! never paints a background — the terminal's own background shows through, so the game blends in.

pub mod results_view;
pub mod theme;
pub mod typing_view;

use ratatui::Frame;

use crate::app::{App, AppState};

/// Top-level render: no background fill; dispatch to the active screen.
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    match app.state {
        AppState::Typing => typing_view::render(frame, app, area),
        AppState::Results => results_view::render(frame, app, area),
    }
}
