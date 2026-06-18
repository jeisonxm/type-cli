//! The rendering layer. Everything here is a pure function of application state into a ratatui
//! `Frame`; it never mutates game state.

pub mod banner;
pub mod results_view;
pub mod theme;
pub mod typing_view;

use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Frame;

use crate::app::{App, AppState};

/// Top-level render: paint the themed background, then dispatch to the active screen.
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    frame.render_widget(
        Block::new().style(Style::new().bg(app.config.theme.bg)),
        area,
    );
    match app.state {
        AppState::Typing => typing_view::render(frame, app, area),
        AppState::Results => results_view::render(frame, app, area),
    }
}
