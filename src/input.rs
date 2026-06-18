//! The crossterm boundary on the input side: map a `KeyEvent` to an engine `Action`.
//!
//! This is the ONLY input-side module that knows about crossterm. The engine consumes `Action`s, so
//! everything downstream (tests, ghost replay, networking) stays terminal-agnostic.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::engine::Action;

/// Translate a key press into an `Action`, or `None` if the key is not bound.
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
    fn plain_char_is_typed() {
        assert_eq!(
            to_action(key(KeyCode::Char('a'), KeyModifiers::NONE)),
            Some(Action::Type('a'))
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
            to_action(key(KeyCode::Char('a'), KeyModifiers::CONTROL)),
            None
        );
    }
}
