//! Pure statistics over a typing session: WPM, accuracy, consistency (monkeytype definitions),
//! and per-key error tallies. Like `engine`, this module reads no clock and touches no IO.

pub mod keystats;
pub mod metrics;

pub use metrics::Summary;
