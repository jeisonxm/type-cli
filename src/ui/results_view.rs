//! The results screen, stealth style: a single discreet line with the run summary, plus a tiny dim
//! hint. No figlet, no splash — it stays unobtrusive. When the timer is visible (Ctrl+T) it also
//! shows a small WPM/sec sparkline; hidden by default to preserve stealth (ADR-0003). Pure render.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Paragraph, Sparkline};
use ratatui::Frame;

use crate::app::App;
use crate::stats::metrics::per_second_raw_wpm;
use crate::stats::Summary;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.config.theme;
    let s = app
        .summary
        .unwrap_or_else(|| Summary::compute(&app.session, app.elapsed()));

    // The WPM/sec series doubles as a discreet sparkline, shown only with the timer (stealth gate).
    let series: Vec<u64> = per_second_raw_wpm(app.session.history())
        .into_iter()
        .map(|w| w.round() as u64)
        .collect();
    let show_spark = app.show_timer && series.len() >= 2;

    let constraints: &[Constraint] = if show_spark {
        &[
            Constraint::Length(1), // summary
            Constraint::Length(1), // sparkline
            Constraint::Length(1), // hint
        ]
    } else {
        &[Constraint::Length(1), Constraint::Length(1)]
    };
    let chunks = Layout::vertical(constraints).margin(1).split(area);

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

    let hint_chunk = if show_spark {
        frame.render_widget(
            Sparkline::default()
                .data(series)
                .style(Style::new().fg(theme.untyped)),
            chunks[1],
        );
        chunks[2]
    } else {
        chunks[1]
    };

    frame.render_widget(
        Paragraph::new("[tab] again · [esc] quit").style(Style::new().fg(theme.untyped)),
        hint_chunk,
    );
}
