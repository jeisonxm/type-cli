# Contributing to type-cli

## Workflow

1. **Read `docs/PROGRESS.md` first** to see the current state (Done / Missing / Now).
2. Work in small, testable increments. Prefer TDD for pure logic (see below).
3. Before committing: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
4. **Update `docs/PROGRESS.md`** at the end of your session (the ritual in `CLAUDE.md`).
5. Use [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`,
   `refactor:`, `test:`, `chore:`).

## Architectural rules (enforced by review)

- **`engine/` and `stats/` are pure**: no `ratatui`/`crossterm`/I/O, and **no `Instant::now()`** —
  time is passed in as `elapsed: Duration`. This keeps tests deterministic and Phase 3 ghost-replay simple.
- **No `crossterm` outside `input.rs` and `main.rs`.** Keystrokes become `Action`s in `input.rs`.
- **`ui/` never mutates state** — it's a pure render of `&App` into a `Frame`.
- Index text by **grapheme**, never by byte.
- Any change to the stack or a core boundary needs a new **ADR** in `docs/adr/`.

## Testing strategy (TDD)

- Pure modules (`engine`, `stats`, `sources::normalizer`) are unit-tested first: write a failing
  test for the rule, then implement it. Tests are colocated in each module under `#[cfg(test)]`.
- `tests/` holds integration tests: full typing sessions, the import normalizer on real fixtures,
  and (from Phase 2) migrations from an empty database.
- Metrics are tested by **injecting `elapsed`** — never by sleeping.
- UI is smoke-tested with ratatui's `TestBackend` where useful.

## Adding things

- **A theme**: drop a `*.toml` in `assets/themes/` (embedded default) or in your config dir (override).
- **A test preset**: edit `[[presets]]` in `config.toml`.
- **A challenge source**: implement the `Source` trait in `src/sources/`.
