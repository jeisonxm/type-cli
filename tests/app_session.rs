//! Integration test: drive a full typing session through `App` and render every screen headlessly
//! with ratatui's `TestBackend`. This exercises the engine → app → ui path end-to-end without a tty.

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
fn typing_a_words_test_reaches_results_and_renders() {
    let mut app = App::new(
        test_config(),
        Mode::Words { count: 3 },
        SourceKind::Random("english".into()),
        None,
    );

    // Type the exact target text.
    let target: String = app.session.target().iter().collect();
    for c in target.chars() {
        app.on_key(press(c));
    }

    assert_eq!(app.state, AppState::Results);
    let summary = app.summary.expect("summary is computed on finish");
    assert!(summary.accuracy > 99.0, "typed everything correctly");
    assert_eq!(summary.incorrect_chars, 0);

    // Render the results screen headlessly and check its labels show up.
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|f| ui::render(f, &app)).unwrap();
    let text = screen_text(&terminal);
    assert!(
        text.contains("accuracy"),
        "results screen shows stat labels"
    );
    assert!(
        text.contains("restart"),
        "results screen shows action hints"
    );
}

#[test]
fn typing_view_renders_without_panic() {
    let app = App::new(
        test_config(),
        Mode::Time { secs: 30 },
        SourceKind::Random("english".into()),
        None,
    );
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal.draw(|f| ui::render(f, &app)).unwrap();
    let text = screen_text(&terminal);
    assert!(text.contains("quit"), "typing screen shows the quit hint");
}

#[test]
fn restart_produces_a_fresh_session() {
    let mut app = App::new(
        test_config(),
        Mode::Words { count: 5 },
        SourceKind::Random("english".into()),
        None,
    );
    app.on_key(press('x')); // one wrong keystroke
    assert!(app.session.is_started());
    app.restart();
    assert_eq!(app.state, AppState::Typing);
    assert!(
        !app.session.is_started(),
        "a fresh session has no keystrokes yet"
    );
    assert!(app.summary.is_none());
}
