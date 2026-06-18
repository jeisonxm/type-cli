//! The crossterm boundary on the input side: map a `KeyEvent` to a `Command`.
//!
//! A `Command` is either an engine `Action` or a UI-level command (handled by the app, not the
//! engine). Keeping all crossterm→intent mapping here means the engine never sees a crossterm type.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::engine::Action;

/// A mapped key press: an engine action, or a UI command the app handles directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Drive the typing engine.
    Engine(Action),
    /// Show/hide the discreet timer (stealth toggle).
    ToggleTimer,
}

/// Translate a key press into a `Command`, or `None` if the key is not bound.
pub fn map_key(key: KeyEvent) -> Option<Command> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    // UI hotkey first: Ctrl+T toggles the timer. It is never a typed character.
    if ctrl && matches!(key.code, KeyCode::Char('t')) {
        return Some(Command::ToggleTimer);
    }
    to_action(key).map(Command::Engine)
}

/// Translate a key press into an engine `Action`, or `None` if unbound.
pub fn to_action(key: KeyEvent) -> Option<Action> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Char('c') if ctrl => Some(Action::Quit),
        KeyCode::Char('w') if ctrl => Some(Action::DeleteWord),
        KeyCode::Backspace if ctrl || alt => Some(Action::DeleteWord),
        KeyCode::Backspace => Some(Action::Backspace),
        KeyCode::Esc => Some(Action::Quit),
        KeyCode::Tab => Some(Action::Restart),
        KeyCode::Enter => Some(Action::Restart),
        // A plain character (no Ctrl) is typed; Ctrl+<other> is ignored.
        KeyCode::Char(c) if !ctrl => Some(Action::Type(c)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn plain_char_maps_to_engine_type() {
        assert_eq!(
            map_key(key(KeyCode::Char('a'), KeyModifiers::NONE)),
            Some(Command::Engine(Action::Type('a')))
        );
    }

    #[test]
    fn ctrl_t_toggles_the_timer() {
        assert_eq!(
            map_key(key(KeyCode::Char('t'), KeyModifiers::CONTROL)),
            Some(Command::ToggleTimer)
        );
        // lowercase t without ctrl is just typed.
        assert_eq!(
            map_key(key(KeyCode::Char('t'), KeyModifiers::NONE)),
            Some(Command::Engine(Action::Type('t')))
        );
    }

    #[test]
    fn ctrl_c_quits_and_ctrl_w_deletes_word() {
        assert_eq!(
            to_action(key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Action::Quit)
        );
        assert_eq!(
            to_action(key(KeyCode::Char('w'), KeyModifiers::CONTROL)),
            Some(Action::DeleteWord)
        );
    }

    #[test]
    fn ctrl_backspace_deletes_word_plain_backspace_deletes_char() {
        assert_eq!(
            to_action(key(KeyCode::Backspace, KeyModifiers::CONTROL)),
            Some(Action::DeleteWord)
        );
        assert_eq!(
            to_action(key(KeyCode::Backspace, KeyModifiers::NONE)),
            Some(Action::Backspace)
        );
    }

    #[test]
    fn unbound_ctrl_combo_is_ignored() {
        assert_eq!(
            map_key(key(KeyCode::Char('a'), KeyModifiers::CONTROL)),
            None
        );
    }
}
