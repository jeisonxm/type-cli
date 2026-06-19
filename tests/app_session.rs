//! Integration test: drive a full typing session through `App` and render every screen headlessly
//! with ratatui's `TestBackend`. Exercises the engine → app → ui path (stealth UI) without a tty.

use std::sync::atomic::{AtomicU64, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::style::Modifier;
use ratatui::Terminal;

use type_cli::app::{App, AppState};
use type_cli::config::{AppConfig, Settings};
use type_cli::engine::Mode;
use type_cli::sources::SourceKind;
use type_cli::storage::Store;
use type_cli::ui;
use type_cli::ui::theme::Theme;

/// Each test gets its own isolated data dir so the persisted SQLite DB never collides across the
/// (parallel) test threads — avoids a first-migration race on a shared file.
fn test_config() -> AppConfig {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let base = std::env::temp_dir().join(format!("type-cli-test-{}-{n}", std::process::id()));
    AppConfig {
        settings: Settings::embedded_default(),
        theme: Theme::fallback(),
        config_dir: base.join("config"),
        data_dir: base.join("data"),
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
fn mistyped_chars_are_never_underlined() {
    // Errors are coloured, never UNDERLINED: the underline modifier makes ratatui emit underline /
    // underline-colour SGR (ESC[4m, ESC[59m) that legacy consoles mis-parse and corrupt. Guard it.
    let mut app = App::new(
        test_config(),
        Mode::Words { count: 3 },
        SourceKind::Random("english".into()),
        None,
        false,
    );
    // Type a wrong character at position 0 (anything different from the expected char).
    let expected = app.session.target()[0];
    let wrong = if expected == 'x' { 'z' } else { 'x' };
    app.on_key(press(wrong));

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal.draw(|f| ui::render(f, &app)).unwrap();

    let buf = terminal.backend().buffer();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            assert!(
                !buf[(x, y)]
                    .style()
                    .add_modifier
                    .contains(Modifier::UNDERLINED),
                "no cell on the typing screen may be underlined (cell {x},{y})"
            );
        }
    }
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

#[test]
fn finished_runs_persist_to_the_database_across_launches() {
    let config = test_config();
    let db_path = config.database_path();

    // Launch 1: play a full 3-word run to completion → it must be persisted on finish.
    {
        let mut app = App::new(
            config.clone(),
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
    } // app dropped → its DB handle is closed (simulates quitting the binary)

    // Launch 2: a brand-new App on the same data dir (simulates relaunching). Must not clobber.
    drop(App::new(
        config.clone(),
        Mode::Words { count: 3 },
        SourceKind::Random("english".into()),
        None,
        false,
    ));

    // The run written in launch 1 survived both the close and the relaunch.
    let store = Store::open(&db_path).expect("reopen db");
    let runs: i64 = store
        .conn()
        .query_row("SELECT COUNT(*) FROM test_run", [], |r| r.get(0))
        .unwrap();
    assert_eq!(runs, 1, "the finished run persisted across launches");
}
