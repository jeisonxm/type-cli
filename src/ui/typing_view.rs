//! The typing screen: a figlet status banner, the per-character colored passage, and a live stats
//! footer. Pure render — reads `&App`, writes the `Frame`.

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::engine::{CharState, Mode};
use crate::stats::Summary;
use crate::ui::banner;
use crate::ui::theme::Theme;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.config.theme;
    let show_banner = app.config.settings.appearance.show_banner;
    let banner_h = if show_banner { 7 } else { 1 };

    let chunks = Layout::vertical([
        Constraint::Length(banner_h),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .margin(1)
    .split(area);

    let elapsed = app.elapsed();
    let summary = Summary::compute(&app.session, elapsed);

    // --- status banner (remaining seconds, or words completed) -------------
    let status = status_text(app, elapsed);
    if show_banner {
        let art = banner::big_text(&status);
        frame.render_widget(
            Paragraph::new(art)
                .alignment(Alignment::Center)
                .style(Style::new().fg(theme.accent)),
            chunks[0],
        );
    } else {
        frame.render_widget(
            Paragraph::new(status)
                .alignment(Alignment::Center)
                .style(Style::new().fg(theme.accent)),
            chunks[0],
        );
    }

    // --- the passage -------------------------------------------------------
    let text_area = chunks[1];
    let (start, end) = visible_window(app, text_area.width, text_area.height);
    let spans: Vec<Span> = (start..end)
        .map(|i| styled_char(app.session.target()[i], app.session.char_state(i), theme))
        .collect();
    frame.render_widget(
        Paragraph::new(Line::from(spans)).wrap(Wrap { trim: false }),
        text_area,
    );

    // --- live stats + hints ------------------------------------------------
    let footer = format!(
        "wpm {:.0}   ·   acc {:.0}%   ·   esc quit   ·   tab restart",
        summary.wpm, summary.accuracy
    );
    frame.render_widget(
        Paragraph::new(footer)
            .alignment(Alignment::Center)
            .style(Style::new().fg(theme.sub)),
        chunks[2],
    );
}

fn status_text(app: &App, elapsed: std::time::Duration) -> String {
    match app.mode {
        Mode::Time { secs } => {
            let remaining = if app.session.is_started() {
                secs.saturating_sub(elapsed.as_secs())
            } else {
                secs
            };
            remaining.to_string()
        }
        Mode::Words { count } => {
            let done = app.session.target()[..app.session.cursor()]
                .iter()
                .filter(|&&c| c == ' ')
                .count();
            format!("{done}/{count}")
        }
    }
}

fn styled_char(ch: char, state: CharState, theme: &Theme) -> Span<'static> {
    let style = match state {
        CharState::Correct => Style::new().fg(theme.correct),
        CharState::Incorrect if ch == ' ' => Style::new().bg(theme.error_bg),
        CharState::Incorrect => Style::new()
            .fg(theme.error)
            .add_modifier(Modifier::UNDERLINED),
        CharState::Caret => Style::new().fg(theme.bg).bg(theme.caret),
        CharState::Untyped => Style::new().fg(theme.untyped),
    };
    Span::styled(ch.to_string(), style)
}

/// Pick a contiguous window of the passage that fits the text area and keeps the caret visible
/// (caret held near the top third). Avoids scroll math while never overflowing on long tests.
fn visible_window(app: &App, width: u16, height: u16) -> (usize, usize) {
    let len = app.session.target().len();
    let capacity = (width as usize * height as usize).max(1);
    if len <= capacity {
        return (0, len);
    }
    let lead = capacity / 3;
    let raw_start = app.session.cursor().saturating_sub(lead);
    let start = align_to_word_start(app.session.target(), raw_start);
    let end = (start + capacity).min(len);
    (start, end)
}

/// Move `idx` back to the start of its word (just after the previous space), or to 0.
fn align_to_word_start(target: &[char], idx: usize) -> usize {
    if idx == 0 {
        return 0;
    }
    match target[..idx].iter().rposition(|&c| c == ' ') {
        Some(space) => space + 1,
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_to_word_start_snaps_to_boundary() {
        let target: Vec<char> = "alpha bravo charlie".chars().collect();
        // index 8 is inside "bravo" (starts at 6) → snaps to 6.
        assert_eq!(align_to_word_start(&target, 8), 6);
        assert_eq!(align_to_word_start(&target, 0), 0);
        assert_eq!(align_to_word_start(&target, 3), 0); // inside first word
    }
}
