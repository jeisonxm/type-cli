//! The `type-cli stats` screen (pure render): a WPM history chart, a bar chart of the most-missed
//! keys, and a QWERTY heatmap coloured by error rate. Unlike the typing/results screens this one is
//! deliberately visual — it is the opt-in analytics view (ADR-0003), shown only on `type-cli stats`.

use std::collections::HashMap;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, BarChart, Block, Chart, Dataset, GraphType, Paragraph};
use ratatui::Frame;

use crate::stats_app::StatsApp;
use crate::ui::theme::Theme;

/// QWERTY rows used by the heatmap (letters only; the stealth themes already cover the colours).
const ROWS: [&str; 3] = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];

pub fn render(frame: &mut Frame, app: &StatsApp, area: Rect) {
    let theme = &app.theme;

    // Empty state: nothing to chart yet.
    if app.run_count == 0 {
        let chunks = Layout::vertical([Constraint::Length(1), Constraint::Length(1)])
            .margin(1)
            .split(area);
        frame.render_widget(
            Paragraph::new("No runs yet — play a test, then come back to `type-cli stats`.")
                .style(Style::new().fg(theme.sub)),
            chunks[0],
        );
        frame.render_widget(
            Paragraph::new("[q] quit").style(Style::new().fg(theme.untyped)),
            chunks[1],
        );
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(1), // title
        Constraint::Min(6),    // wpm chart
        Constraint::Length(9), // most-missed keys bar chart
        Constraint::Length(5), // qwerty heatmap (3 rows + title + pad)
        Constraint::Length(1), // hint
    ])
    .margin(1)
    .split(area);

    frame.render_widget(
        Paragraph::new(format!("type-cli stats · {} runs", app.run_count))
            .style(Style::new().fg(theme.accent)),
        chunks[0],
    );

    render_wpm_chart(frame, app, chunks[1]);
    render_missed_keys(frame, app, chunks[2]);
    render_heatmap(frame, app, chunks[3]);

    let hint = if app.can_practice() {
        "[r] practice slow letters · [o] period · [O] type · ←/→ scroll · [q] quit"
    } else {
        "[o] period · [O] type · ←/→ scroll · [q] quit"
    };
    frame.render_widget(
        Paragraph::new(hint).style(Style::new().fg(theme.untyped)),
        chunks[4],
    );
}

fn render_wpm_chart(frame: &mut Frame, app: &StatsApp, area: Rect) {
    let theme = &app.theme;
    // The visible window of the currently-selected (period, filter) series.
    let series = app.visible_series();
    let max_wpm = series.iter().map(|(_, y)| *y).fold(1.0_f64, f64::max);
    let last_x = (series.len().saturating_sub(1)).max(1) as f64;

    let dataset = Dataset::default()
        .name(app.graph_filter.label())
        .marker(symbols::Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::new().fg(theme.accent))
        .data(&series);

    let title = format!("wpm · {}", app.graph_title());
    let chart = Chart::new(vec![dataset])
        .block(Block::default().title(Span::styled(title, Style::new().fg(theme.sub))))
        .x_axis(Axis::default().bounds([0.0, last_x]))
        .y_axis(
            Axis::default()
                .style(Style::new().fg(theme.untyped))
                .bounds([0.0, max_wpm * 1.15])
                .labels([Span::raw("0"), Span::raw(format!("{max_wpm:.0}"))]),
        );
    frame.render_widget(chart, area);
}

fn render_missed_keys(frame: &mut Frame, app: &StatsApp, area: Rect) {
    let theme = &app.theme;
    let bars: Vec<(String, u64)> = app
        .key_aggs
        .iter()
        .filter(|a| a.error_count > 0)
        .take(10)
        .map(|a| (a.ch.clone(), a.error_count as u64))
        .collect();

    if bars.is_empty() {
        frame.render_widget(
            Paragraph::new("most-missed keys: none yet (clean runs!)")
                .style(Style::new().fg(theme.sub)),
            area,
        );
        return;
    }

    let data: Vec<(&str, u64)> = bars.iter().map(|(s, n)| (s.as_str(), *n)).collect();
    let chart = BarChart::default()
        .block(Block::default().title(Span::styled(
            "most-missed keys (errors)",
            Style::new().fg(theme.sub),
        )))
        .data(&data[..])
        .bar_width(3)
        .bar_gap(1)
        .bar_style(Style::new().fg(theme.error))
        .value_style(Style::new().fg(theme.bg).bg(theme.error))
        .label_style(Style::new().fg(theme.untyped));
    frame.render_widget(chart, area);
}

fn render_heatmap(frame: &mut Frame, app: &StatsApp, area: Rect) {
    let theme = &app.theme;
    // Map each expected character to its error rate.
    let mut rate: HashMap<char, f64> = HashMap::new();
    for agg in &app.key_aggs {
        if let Some(c) = agg.ch.chars().next() {
            rate.insert(c, agg.error_rate());
        }
    }

    let mut lines: Vec<Line> = Vec::with_capacity(ROWS.len() + 1);
    lines.push(Line::from(Span::styled(
        "keyboard heatmap (dim = good, red = missed)",
        Style::new().fg(theme.sub),
    )));
    for row in ROWS {
        let mut spans: Vec<Span> = Vec::new();
        for c in row.chars() {
            let color = match rate.get(&c) {
                None => theme.untyped, // no data → dim
                Some(&r) => heat_color(theme, r),
            };
            spans.push(Span::styled(
                format!("{} ", c.to_ascii_uppercase()),
                Style::new().fg(color),
            ));
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

/// Bucket an error rate into a theme colour: clean → correct, some → accent, frequent → error.
fn heat_color(theme: &Theme, rate: f64) -> Color {
    if rate <= 0.02 {
        theme.correct
    } else if rate <= 0.08 {
        theme.accent
    } else {
        theme.error
    }
}
