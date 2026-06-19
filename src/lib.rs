//! type-cli — a fast monkeytype-style touch-typing game for the terminal.
//!
//! The crate is split into a **pure core** (`engine`, `stats`) that has zero terminal/IO/clock
//! dependencies, and an **impure shell** (added in later steps: `input`, `ui`, `config`, `sources`)
//! that wires the core to crossterm, ratatui, files and the clock.
//!
//! See `docs/ARCHITECTURE.md` and `CLAUDE.md` for the invariants that keep this boundary clean.

pub mod app;
pub mod cli;
pub mod config;
pub mod engine;
pub mod input;
pub mod sources;
pub mod stats;
pub mod storage;
pub mod ui;
