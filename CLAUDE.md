# CLAUDE.md — operating manual for AI agents working on type-cli

> **type-cli** is a fast, monkeytype-style touch-typing game for the terminal (Rust + ratatui).
> This file is the agent's quick brief. It is intentionally short and links out.

## First thing every session

1. **Read [`docs/PROGRESS.md`](docs/PROGRESS.md)** — the living state file (Done / Missing / Now).
   It answers "where are we?" without re-deriving from code.
2. Skim [`docs/ROADMAP.md`](docs/ROADMAP.md) for the phase you're in and its acceptance criteria.
3. For the "why" of any design, see [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) and [`docs/adr/`](docs/adr/).

## Last thing every session (the ritual — do NOT skip)

Update **`docs/PROGRESS.md`**:
- Move finished items to **Done** (with date + short commit ref).
- Re-sort **Missing** (next concrete tasks, each with its acceptance hook).
- Rewrite **Now** (current phase, in-flight task, blockers, last decision).

This keeps "lo que se crea / lo que hace falta / en lo que vamos" always current.

## Commands

```bash
. "$HOME/.cargo/env"            # cargo isn't auto on PATH in non-interactive shells
cargo build
cargo test                     # unit + integration; pure modules are the bulk
cargo clippy --all-targets -- -D warnings
cargo fmt --all
cargo run -- --time 60         # play a 60s test
cargo run -- import file.pdf   # build a challenge from a document
```

## Locked decisions (do NOT change without an ADR)

The stack (ratatui+crossterm, rusqlite bundled [P2], pdf-extract + zip/quick-xml,
directories+toml+serde, clap, anyhow/thiserror) is recorded in [`docs/adr/ADR-0001-tech-stack.md`](docs/adr/ADR-0001-tech-stack.md).
Changing the stack or a core architectural boundary requires a new ADR.

The UI is **stealth-only** (terminal-native camouflage; no figlet/background/chrome) — see
[`docs/adr/ADR-0002-stealth-ui.md`](docs/adr/ADR-0002-stealth-ui.md). The mission: look like normal
terminal work while practicing touch-typing.

## Non-negotiable invariants (these prevent the project's worst bugs)

1. **The `engine` and `stats` modules are PURE.** No `ratatui`, no `crossterm`, no I/O, and
   **never call `Instant::now()` inside them** — time enters as an `elapsed: Duration` parameter.
   This is what makes metrics deterministic in tests and makes Phase 3 ghost-replay trivial.
2. **Input flows as `KeyEvent → Action` in `input.rs`**, then the engine consumes `Action`s.
   The engine has zero `crossterm` dependency.
3. **`ui/` is a pure function of state → Frame.** It never mutates game state.
4. **Filter `KeyEventKind::Press`.** Windows + the kitty protocol also emit Repeat/Release;
   counting them double-fires keystrokes and corrupts WPM/accuracy. This is bug #1.
5. **RAII teardown + panic hook are mandatory.** A panic in raw mode must still restore the
   terminal (disable raw mode, leave alternate screen, pop keyboard-enhancement flags).
6. **Index by grapheme, not byte.** Imported text (PDF/DOCX) contains accents/emoji.

## Conventions

- rustfmt + clippy must be clean (`-D warnings`). Conventional Commits.
- See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the full workflow.
