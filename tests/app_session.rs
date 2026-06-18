//! Integration test: drive a full typing session through `App` and render every screen headlessly
//! with ratatui's `TestBackend`. Exercises the engine → app → ui path (stealth UI) without a tty.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use type_cli::app::{App, AppState};
use type_cli::config::{AppConfig, Settings};
use type_cli::engine::Mode;
use type_cli::sources::SourceKind;
use type_cli::ui;
use type_cli::ui::theme::Theme;

fn test_config() -> AppConfig {
    AppConfig {
        settings: Settings::embedded_default(),
        theme: Theme::fallback(),
        config_dir: PathBuf::from("/tmp/type-cli-test/config"),
        data_dir: PathBuf::from("/tmp/type-cli-test/data"),
    }
}

fn press(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

fn screen_text(terminal: &Terminal<TestBackend>) -> String {
    let buf = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            out.push_str(buf[(x, y)].symbol());
        }
    }
    out
}

#[test]
fn typing_a_words_test_reaches_results_with_a_one_line_summary() {
    let mut app = App::new(
        test_config(),
        Mode::Words { count: 3 },
        SourceKind::Random("english".into()),
        None,
        false,
    );

    let target: String = app.session.target().iter().collect();
    for c in target.chars() {
        app.on_key(press(c));
    }

    assert_eq!(app.state, AppState::Results);
    let summary = app.summary.expect("summary is computed on finish");
    assert!(summary.accuracy > 99.0, "typed everything correctly");
    assert_eq!(summary.incorrect_chars, 0);

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|f| ui::render(f, &app)).unwrap();
    let text = screen_text(&terminal);
    assert!(text.contains("wpm"), "results shows the one-line summary");
    assert!(text.contains("acc"), "results shows accuracy");
}

#[test]
fn typing_screen_is_plain_text_with_no_chrome() {
    let app = App::new(
        test_config(),
        Mode::Time { secs: 30 },
        SourceKind::Random("english".into()),
        None,
        false, // timer hidden
    );
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|f| ui::render(f, &app)).unwrap();
    let text = screen_text(&terminal);

    // The passage's first word is visible…
    let target: String = app.session.target().iter().collect();
    let first_word = target.split(' ').next().unwrap();
    assert!(text.contains(first_word), "the passage is rendered");
    // …and there is no game chrome: no stats, no hints, no timer.
    assert!(!text.contains("wpm"), "no stats on the typing screen");
    assert!(
        !text.contains("quit"),
        "no hint chrome on the typing screen"
    );
}

#[test]
fn ctrl_t_toggles_the_discreet_timer() {
    let mut app = App::new(
        test_config(),
        Mode::Time { secs: 60 },
        SourceKind::Random("english".into()),
        None,
        false,
    );
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();

    // Hidden: no timer.
    terminal.draw(|f| ui::render(f, &app)).unwrap();
    assert!(!screen_text(&terminal).contains("60s"));

    // Toggle on with Ctrl+T → the timer shows the FULL duration (test hasn't started typing yet).
    app.on_key(ctrl('t'));
    assert!(app.show_timer);
    terminal.draw(|f| ui::render(f, &app)).unwrap();
    assert!(
        screen_text(&terminal).contains("60s"),
        "timer shows full duration before the first keystroke"
    );
}

#[test]
fn restart_produces_a_fresh_session() {
    let mut app = App::new(
        test_config(),
        Mode::Words { count: 5 },
        SourceKind::Random("english".into()),
        None,
        false,
    );
    app.on_key(press('x')); // one keystroke
    assert!(app.session.is_started());
    app.restart();
    assert_eq!(app.state, AppState::Typing);
    assert!(
        !app.session.is_started(),
        "a fresh session has no keystrokes yet"
    );
    assert!(app.summary.is_none());
}
