//! The pure typing engine: game state and the rules that advance it.
//!
//! INVARIANT: nothing here may depend on `ratatui`, `crossterm`, the filesystem, or read the
//! clock. Time enters as an `elapsed: Duration` parameter. This is what makes the engine
//! deterministic in tests and makes Phase 3 ghost-replay a trivial re-feed of recorded actions.

pub mod action;
pub mod presets;
pub mod session;

pub use action::Action;
pub use presets::Mode;
pub use session::{CharState, Keystroke, Slot, TypingSession};
