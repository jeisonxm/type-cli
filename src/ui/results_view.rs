//! The results screen, stealth style: a single discreet line with the run summary, plus a tiny dim
//! hint. No figlet, no splash — it stays unobtrusive. Pure render.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::stats::Summary;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.config.theme;
    let s = app
        .summary
        .unwrap_or_else(|| Summary::compute(&app.session, app.elapsed()));

    let chunks = Layout::vertical([Constraint::Length(1), Constraint::Length(1)])
        .margin(1)
        .split(area);

    let summary = format!(
        "{:.0} wpm · {:.0}% acc · {:.0}% con · {:.1}s",
        s.wpm,
        s.accuracy,
        s.consistency,
        s.elapsed.as_secs_f64()
    );
    frame.render_widget(
        Paragraph::new(summary).style(Style::new().fg(theme.sub)),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new("[tab] again · [esc] quit").style(Style::new().fg(theme.untyped)),
        chunks[1],
    );
}
