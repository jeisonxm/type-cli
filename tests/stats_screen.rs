//! Integration test: render the `type-cli stats` screen headlessly from a database seeded with
//! synthetic runs. Exercises storage queries → StatsApp → ui::stats_view without a tty.

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use type_cli::stats_app::StatsApp;
use type_cli::storage::{insert_run, CharStatRow, RunRecord, Store, WorstWordRow};
use type_cli::ui::stats_view;
use type_cli::ui::theme::Theme;

fn run(started_at: i64, wpm: f64) -> RunRecord {
    RunRecord {
        mode: "time",
        target: 60,
        source: "random",
        source_ref: None,
        language: Some("english".into()),
        wpm,
        raw_wpm: wpm + 4.0,
        accuracy: 96.0,
        consistency: Some(91.0),
        chars_correct: 210,
        chars_incorrect: 6,
        chars_extra: 0,
        chars_missed: 1,
        elapsed_ms: 60_000,
        started_at,
        created_at: started_at + 60_000,
        char_stats: vec![CharStatRow {
            expected_char: "e".into(),
            typed_total: 40,
            error_count: 9,
        }],
        worst_words: vec![WorstWordRow {
            word: "their".into(),
            error_count: 2,
            word_wpm: Some(38.0),
            rank: 1,
        }],
    }
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
fn stats_screen_renders_history_keys_and_heatmap() {
    let mut store = Store::open_in_memory().unwrap();
    insert_run(&mut store, &run(100, 70.0)).unwrap();
    insert_run(&mut store, &run(200, 85.0)).unwrap();

    let app = StatsApp::load(&store, &Theme::fallback()).unwrap();
    assert!(app.can_retry(), "the latest run had a worst word");

    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|f| {
            let area = f.area();
            stats_view::render(f, &app, area);
        })
        .unwrap();
    let text = screen_text(&terminal);

    assert!(text.contains("type-cli stats"), "title with run count");
    assert!(text.contains("2 runs"));
    assert!(text.contains("wpm over time"), "history chart present");
    assert!(text.contains("most-missed keys"), "bar chart present");
    assert!(text.contains("heatmap"), "qwerty heatmap present");
    assert!(
        text.contains("retry worst words"),
        "retry hint shown when a worst word exists"
    );
}

#[test]
fn stats_screen_shows_empty_state_with_no_runs() {
    let store = Store::open_in_memory().unwrap();
    let app = StatsApp::load(&store, &Theme::fallback()).unwrap();
    assert!(!app.can_retry());

    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|f| {
            let area = f.area();
            stats_view::render(f, &app, area);
        })
        .unwrap();
    let text = screen_text(&terminal);
    assert!(text.contains("No runs yet"), "empty-state message");
    assert!(!text.contains("retry"), "no retry hint with no data");
}
