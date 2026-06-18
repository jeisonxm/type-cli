//! `Action` — the engine's entire input vocabulary.
//!
//! `input.rs` maps crossterm `KeyEvent`s to these; the engine consumes only `Action`s and never
//! sees a crossterm type. The same path is reused by unit tests, Phase 3 ghost-replay, and Phase 4
//! networking — anything that can produce `Action`s can drive the game.

/// A single intent produced by the player (or by a replay / the network).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// A typed character. Space is included here and handled specially by the session
    /// (space mid-word skips to the next word).
    Type(char),
    /// Delete the previous character.
    Backspace,
    /// Delete the current word back to the previous word boundary (Ctrl/Alt+Backspace).
    DeleteWord,
    /// Restart the current test from scratch. Handled by the app layer.
    Restart,
    /// Quit to the menu / exit. Handled by the app layer.
    Quit,
}
