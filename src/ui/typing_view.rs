//! The typing screen, stealth style: plain text aligned top-left like a normal terminal, upcoming
//! text dimmed (reads like a shell autosuggestion), with an optional discreet timer. Pure render.

use std::time::Duration;

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::engine::{CharState, Mode};
use crate::ui::theme::Theme;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let theme = &app.config.theme;

    // Reserve a thin bottom row for the timer only when it is visible.
    let (text_area, timer_area) = if app.show_timer {
        let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)])
            .margin(1)
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        let chunks = Layout::vertical([Constraint::Min(1)]).margin(1).split(area);
        (chunks[0], None)
    };

    // The passage: a window that fits the area, rendered top-left.
    let (start, end) = visible_window(app, text_area.width, text_area.height);
    let spans: Vec<Span> = (start..end)
        .map(|i| styled_char(app.session.target()[i], app.session.char_state(i), theme))
        .collect();
    frame.render_widget(
        Paragraph::new(Line::from(spans))
            .wrap(Wrap { trim: false })
            .alignment(Alignment::Left),
        text_area,
    );

    // Discreet timer, bottom-right, dim.
    if let Some(rect) = timer_area {
        let typing_elapsed = app.session.elapsed(app.elapsed());
        let label = timer_label(
            app.mode,
            typing_elapsed,
            app.session.is_started(),
            word_progress(app),
        );
        frame.render_widget(
            Paragraph::new(label)
                .alignment(Alignment::Right)
                .style(Style::new().fg(theme.sub)),
            rect,
        );
    }
}

/// The timer/progress label. Pure and unit-testable. Time counts from the first keystroke: before
/// the test starts it shows the full duration regardless of how long the screen has been open.
fn timer_label(mode: Mode, typing_elapsed: Duration, started: bool, words_done: usize) -> String {
    match mode {
        Mode::Time { secs } => {
            let left = if started {
                secs.saturating_sub(typing_elapsed.as_secs())
            } else {
                secs
            };
            format!("{left}s")
        }
        Mode::Words { count } => format!("{words_done}/{count}"),
    }
}

fn word_progress(app: &App) -> usize {
    app.session.target()[..app.session.cursor()]
        .iter()
        .filter(|&&c| c == ' ')
        .count()
}

fn styled_char(ch: char, state: CharState, theme: &Theme) -> Span<'static> {
    let style = match state {
        CharState::Correct => Style::new().fg(theme.correct),
        CharState::Incorrect => Style::new()
            .fg(theme.error)
            .add_modifier(Modifier::UNDERLINED),
        // A reversed cell mimics the terminal's own block cursor.
        CharState::Caret => Style::new().add_modifier(Modifier::REVERSED),
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

    #[test]
    fn timer_counts_from_first_keystroke_not_screen_open() {
        // Before the test starts, it must show the full duration even if the screen has been open.
        assert_eq!(
            timer_label(Mode::Time { secs: 60 }, Duration::from_secs(30), false, 0),
            "60s"
        );
        // Once started, it counts down from the typing-elapsed time.
        assert_eq!(
            timer_label(Mode::Time { secs: 60 }, Duration::from_secs(10), true, 0),
            "50s"
        );
        assert_eq!(
            timer_label(Mode::Words { count: 100 }, Duration::from_secs(5), true, 12),
            "12/100"
        );
    }
}
