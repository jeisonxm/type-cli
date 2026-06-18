//! The results screen: a big figlet WPM, a row of stat cards, and action hints. Pure render.

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::stats::Summary;
use crate::ui::banner;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.config.theme;
    let s = app
        .summary
        .unwrap_or_else(|| Summary::compute(&app.session, app.elapsed()));

    let chunks = Layout::vertical([
        Constraint::Min(7),    // big WPM banner
        Constraint::Length(3), // stat cards
        Constraint::Length(2), // hints
    ])
    .margin(1)
    .split(area);

    // --- big WPM -----------------------------------------------------------
    let art = banner::big_text(&format!("{:.0}", s.wpm));
    frame.render_widget(
        Paragraph::new(art)
            .alignment(Alignment::Center)
            .style(Style::new().fg(theme.accent)),
        chunks[0],
    );

    // --- stat cards --------------------------------------------------------
    let cards = Layout::horizontal([Constraint::Ratio(1, 4); 4]).split(chunks[1]);
    let entries = [
        ("accuracy", format!("{:.0}%", s.accuracy)),
        ("consistency", format!("{:.0}%", s.consistency)),
        ("raw wpm", format!("{:.0}", s.raw_wpm)),
        ("time", format!("{:.1}s", s.elapsed.as_secs_f64())),
    ];
    for (rect, (label, value)) in cards.iter().zip(entries) {
        frame.render_widget(stat_card(label, &value, theme.correct, theme.sub), *rect);
    }

    // --- hints -------------------------------------------------------------
    frame.render_widget(
        Paragraph::new("tab / enter restart      ·      q / esc quit")
            .alignment(Alignment::Center)
            .style(Style::new().fg(theme.sub)),
        chunks[2],
    );
}

fn stat_card(
    label: &str,
    value: &str,
    value_color: Color,
    label_color: Color,
) -> Paragraph<'static> {
    Paragraph::new(vec![
        Line::from(Span::styled(
            value.to_string(),
            Style::new().fg(value_color).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            label.to_string(),
            Style::new().fg(label_color),
        )),
    ])
    .alignment(Alignment::Center)
}
