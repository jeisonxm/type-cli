# ADR-0001 — Technology stack

- **Status:** Accepted
- **Date:** 2026-06-18
- **Deciders:** Jeison + agent

## Context

type-cli is a terminal touch-typing game (monkeytype-in-the-terminal). It must have minimal
keystroke→paint latency, ship as a single portable binary, persist records for charts/stats, import
challenges from PDF/Word, and grow toward ghost-replay and online play. We needed to lock the
language and the crate for each concern before building.

## Decision

**Language:** Rust.

| Concern | Choice | Rationale |
|---|---|---|
| TUI | `ratatui` 0.30 | De-facto Rust TUI lib; built-in Chart/Sparkline/BarChart/Canvas cover all Phase 2 views — no extra chart crate. |
| Terminal backend / input | `crossterm` 0.29 | Cross-platform (Linux/macOS/Windows), kitty keyboard protocol, best documented. |
| ASCII banners | `figlet-rs` 0.1.x (pin) | Pure Rust, no C deps; `FIGfont::from_content(include_str!())`. |
| SQLite | `rusqlite` 0.40 `["bundled"]` | Synchronous FFI, SQLite compiled in → single binary, lowest latency. (Introduced in Phase 2.) |
| DB migrations | `rusqlite_migration` 2.6 | State in `PRAGMA user_version`, no metadata table, inline Rust migrations. |
| Config + serialization | `serde` 1 + `toml` 0.8 | `ratatui::Color` already (de)serializes hex/name/index. |
| XDG dirs | `directories` 6 | Correct per-OS config/data dirs via `ProjectDirs`. |
| PDF extraction | `pdf-extract` 0.10 | Pure Rust, zero native deps, one-call `extract_text()`. |
| DOCX extraction | `zip` 2 + `quick-xml` 0.37 | Pure Rust, full whitespace control. |
| Unicode | `unicode-segmentation` / `-width` / `-normalization` | Grapheme-safe cursor, column math, NFC. |
| CLI | `clap` 4 (derive) | Subcommands + flags. |
| Errors | `anyhow` (app) + `thiserror` (libs) | Typed library errors, ergonomic app glue. |
| Randomness | `rand` 0.9 | Passage / word selection. |

## Alternatives considered & rejected

- **Go + Bubble Tea / TypeScript + Bun / Python + Textual** — viable, but Rust gives the lowest
  per-keystroke latency and the best single-binary story for the online phase. (Python had the worst
  input latency — a real risk for a typing game.)
- **termion / termwiz** (vs crossterm) — termion is Unix-only; termwiz is heavier and WezTerm-centric.
- **sqlx** (vs rusqlite) — async runtime + compile-time-checked queries buy nothing for a local,
  single-user, latency-sensitive app and add a Tokio runtime + build-time DB.
- **pdfium-render** (vs pdf-extract) — best fidelity but needs a native pdfium lib → breaks the
  single-binary goal. Kept as an opt-in `--features pdfium`.
- **docx-rs** (write-only) and **dotext** (unmaintained since 2017) — rejected for DOCX reading.
- **figlet-rs 1.x line** — a divergent high-numbered lineage with a different API; we pin the 0.1.x line.
- **A third-party chart crate** — unnecessary; ratatui built-ins suffice.

## Consequences

- One portable binary; assets (themes, fonts, default config, wordlists) embedded via `include_str!`.
- The pure `engine`/`stats` boundary (time injected as `elapsed`) makes metrics testable and Phase 3
  ghost-replay trivial.
- Known pitfalls to guard (encoded in `CLAUDE.md`): filter `KeyEventKind::Press`; RAII teardown +
  panic hook; cache spans on long tests; grapheme (not byte) indexing; gate kitty flags behind
  `supports_keyboard_enhancement()`; ship a 16-color fallback theme.

## Future ADRs

- ADR-0002 — event-loop model (when revisited for Phase 4 networking).
- ADR-0003 — engine purity contract (if challenged).
- ADR-0004 — ghost storage: row `keystroke_event` vs compressed BLOB (decided at Phase 3 start).
- ADR-0005 — online sync protocol/auth/hosting (Phase 4 design spike).
