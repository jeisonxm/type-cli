//! type-cli binary: parse the CLI, set up the terminal (with a panic hook + RAII teardown so a crash
//! never leaves the terminal in raw mode), and run the event-driven loop.

use std::io::{self, Stdout};
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::event::{
    self, Event, KeyEventKind, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement, EnterAlternateScreen,
    LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use type_cli::app::App;
use type_cli::cli::{Cli, Command};
use type_cli::config::AppConfig;
use type_cli::engine::Mode;
use type_cli::sources::{self, SourceKind};
use type_cli::ui;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut config = AppConfig::load();
    if let Some(theme) = &cli.theme {
        config = config.with_theme(theme);
    }

    match &cli.command {
        Some(Command::Config) => {
            print_config(&config);
            Ok(())
        }
        Some(Command::Theme) => {
            print_themes(&config);
            Ok(())
        }
        Some(Command::Import { path }) => {
            let text = sources::load_document(path, config.settings.game.fold_smart_punctuation)?;
            let mode = resolve_mode(&cli, &config);
            let source = match path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_lowercase)
                .as_deref()
            {
                Some("pdf") => SourceKind::Pdf(path.clone()),
                _ => SourceKind::Docx(path.clone()),
            };
            run_tui(App::new(config, mode, source, Some(text)))
        }
        None => {
            let mode = resolve_mode(&cli, &config);
            let lang = config.settings.game.language.clone();
            run_tui(App::new(config, mode, SourceKind::Random(lang), None))
        }
    }
}

/// Decide the test mode from flags, falling back to the configured default preset.
fn resolve_mode(cli: &Cli, config: &AppConfig) -> Mode {
    if let Some(secs) = cli.time {
        Mode::Time { secs }
    } else if let Some(count) = cli.words {
        Mode::Words { count }
    } else if let Some(preset) = &cli.preset {
        config
            .settings
            .mode_for(preset)
            .unwrap_or_else(|| config.settings.default_mode())
    } else {
        config.settings.default_mode()
    }
}

fn print_config(config: &AppConfig) {
    println!("config dir : {}", config.config_dir.display());
    println!("data dir   : {}", config.data_dir.display());
    println!("database   : {}", config.database_path().display());
    println!("theme      : {}", config.theme.name);
    println!("default    : {:?}", config.settings.default_mode());
}

fn print_themes(config: &AppConfig) {
    println!("Built-in themes: serika_dark, dracula, classic16");
    let dir = config.config_dir.join("themes");
    if let Ok(entries) = std::fs::read_dir(&dir) {
        println!("User themes in {}:", dir.display());
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    println!("  {name}");
                }
            }
        }
    }
    println!("Active theme   : {}", config.theme.name);
}

// --- terminal lifecycle -----------------------------------------------------

/// Owns the terminal in raw/alternate-screen mode and restores it on drop (every exit path).
struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    kitty: bool,
}

impl Tui {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        let mut out = io::stdout();
        execute!(out, EnterAlternateScreen)?;
        // Best-effort kitty keyboard protocol (disambiguate escape codes); gated by support.
        let kitty = matches!(supports_keyboard_enhancement(), Ok(true));
        if kitty {
            let _ = execute!(
                out,
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            );
        }
        let terminal = Terminal::new(CrosstermBackend::new(out))?;
        Ok(Tui { terminal, kitty })
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let mut out = io::stdout();
        if self.kitty {
            let _ = execute!(out, PopKeyboardEnhancementFlags);
        }
        let _ = execute!(out, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

/// Install a panic hook that restores the terminal before the default panic message prints, so a
/// panic in raw mode never leaves the user's terminal wrecked.
fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mut out = io::stdout();
        let _ = execute!(out, PopKeyboardEnhancementFlags);
        let _ = execute!(out, LeaveAlternateScreen);
        let _ = disable_raw_mode();
        original(info);
    }));
}

fn run_tui(mut app: App) -> Result<()> {
    install_panic_hook();
    let mut tui = Tui::enter()?;
    let result = run_loop(&mut app, &mut tui.terminal);
    drop(tui); // restore the terminal before returning (so any error prints cleanly)
    result
}

/// The event-driven loop: block on input up to a ~10 Hz tick (which advances the timer), redraw,
/// repeat. Only `KeyEventKind::Press` is processed — Repeat/Release would double-count keystrokes.
fn run_loop(app: &mut App, terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    let tick = Duration::from_millis(100);
    while !app.should_quit {
        app.on_tick();
        terminal.draw(|frame| ui::render(frame, app))?;
        if event::poll(tick)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.on_key(key);
                }
            }
        }
    }
    Ok(())
}
