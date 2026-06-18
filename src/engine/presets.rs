//! Test modes. A `Mode` is what the engine needs to know to decide when a test ends.
//!
//! The user-facing, editable preset list (ids like `time_60`, `words_100`) lives in `config.toml`
//! and is parsed in `config.rs`; it resolves down to one of these `Mode`s.

/// How a test terminates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Run for a fixed number of seconds, then stop (challenge text is a long buffer).
    Time { secs: u64 },
    /// Run until a fixed number of words has been typed.
    Words { count: usize },
}

impl Mode {
    /// A short label for banners / results (e.g. `"60s"`, `"100w"`).
    pub fn label(&self) -> String {
        match self {
            Mode::Time { secs } => format!("{secs}s"),
            Mode::Words { count } => format!("{count}w"),
        }
    }
}
