//! Command-line interface (clap). The default invocation starts a typing test; subcommands import a
//! document or print diagnostics.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "type-cli",
    version,
    about = "A fast monkeytype-style touch-typing game for the terminal"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Run a timed test of N seconds (e.g. --time 60).
    #[arg(long, global = true, value_name = "SECONDS")]
    pub time: Option<u64>,

    /// Run a word-count test of N words (e.g. --words 100).
    #[arg(long, global = true, value_name = "WORDS")]
    pub words: Option<usize>,

    /// Use a preset id from your config (e.g. --preset time_30).
    #[arg(long, global = true, value_name = "ID")]
    pub preset: Option<String>,

    /// Override the color theme by name (e.g. --theme dracula).
    #[arg(long, global = true, value_name = "NAME")]
    pub theme: Option<String>,

    /// Start with the discreet timer visible (toggle anytime in-game with Ctrl+T).
    #[arg(long, global = true)]
    pub show_timer: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Import a PDF or Word (.docx) file and type its text.
    Import {
        /// Path to the .pdf or .docx file.
        path: PathBuf,
    },
    /// Print the resolved config/data paths and active settings.
    Config,
    /// List the available color themes.
    Theme,
    /// Show your typing history, most-missed keys, and a keyboard heatmap.
    Stats,
}
