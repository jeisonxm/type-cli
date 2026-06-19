# Changelog

All notable changes to this project are documented here.
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.3.0] - 2026-06-19

### Changed — stats follow-up (practice by slowest letters)
- **Practice now drills your slowest letters** instead of the words you mistyped. Per-letter typing
  latency (clean letter-to-letter digraphs, word-initial/punctuation predecessors excluded) is
  persisted (additive migration 2 on `char_stat`) and aggregated across your whole history; pressing
  `r` in `type-cli stats` builds a drill from words rich in those letters (Unicode-aware accent
  folding for non-English). Replaces the old "retry worst words" drill.
- **Practice runs no longer pollute your stats.** Drill runs stay tagged `source='retry'` but are
  excluded from every analytics query (run count, history graph, key heatmap, slowest-letters), so
  practicing never skews your real numbers — this also retroactively cleans already-saved drills.

### Added — graph navigation
- The WPM history graph gained keys in `type-cli stats`: **`o`** cycles the aggregation period
  (session / day / week / month / year — averaged per bucket), **`O`** cycles a test-type filter
  (all / time / words), and **←/→** scroll through history when there are many attempts. Date
  bucketing uses SQLite (no new dependency); week grouping is calendar-based (`%W`).

## [0.2.0] - 2026-06-19

### Added — Phase 2 (persistence + stats/charts)
- **SQLite persistence** via `rusqlite` (bundled) + `rusqlite_migration`. New impure `storage/`
  module (kept out of the pure engine/stats core): WAL/NORMAL/foreign-keys pragmas, an idempotent
  migration with the full schema, and a seeded `local` user. Every finished run is saved with its
  per-character tallies and worst words, in one transaction. Persistence is best-effort — a
  missing/locked DB disables saving but never crashes the game.
- **`type-cli stats`** — an opt-in analytics TUI: a WPM-over-time `Chart`, a `BarChart` of the
  most-missed keys, and a colour-coded QWERTY heatmap (min-sample gated). Empty-state when there's
  no history yet.
- **Retry worst words** — press `r` in the stats screen to start a drill seeded from the latest
  run's worst words (`SourceKind::Retry`, persisted as `source='retry'`).
- **WPM/sec sparkline** on the results screen, gated behind the timer toggle (Ctrl+T) so results
  stay stealth by default.
- Pure stats extended: `Summary` gains `missed_chars`/`extra_chars`; `stats::keystats::worst_words`
  ranks mistyped words with per-word WPM.

### Notes
- **ADR-0003** records that the stats visualization is a bounded, opt-in exception to the stealth UI
  (ADR-0002): the typing screen and default results stay stealth; rich visuals appear only on
  explicit `type-cli stats` (or with the timer toggled on).

## [0.1.3] - 2026-06-19

### Changed
- **Mistyped characters are now signalled by colour alone (no underline).** The `UNDERLINED` modifier
  made ratatui emit the underline SGR `ESC[4m` on every error cell; dropping it removes that escape,
  simplifies the error signal, and is more stealth. (Defence-in-depth / lower escape surface on
  minimal consoles — the actual "frozen terminal after a mistype" bug was the cursor not being
  restored on exit, fixed in 0.1.2. The `ESC[39m ESC[49m ESC[59m ESC[0m` reset tail seen in that
  leak is ratatui's normal per-frame reset, emitted regardless of styling.)

## [0.1.2] - 2026-06-19

### Fixed
- **Terminal cursor was left hidden after exit**, which looked like a frozen terminal (e.g. after a
  mistype). ratatui hides the cursor on every frame and the teardown never re-showed it; it now
  restores the cursor (`Show`) on both the RAII-drop and panic-hook paths. The teardown is factored
  into a unit-tested `restore_terminal` helper so the cursor-restore contract can't silently regress.

## [0.1.1] - 2026-06-18

### Changed — Stealth UI (ADR-0002)
- The UI is now **terminal-native camouflage**: no figlet banners, no background fill, no chrome.
  The typing screen is plain top-left text (upcoming text dimmed like a shell autosuggestion).
- New default theme `terminal` (uses the terminal's own colors via `reset`).
- The timer is **hidden by default**; toggle it in-game with **Ctrl+T** (or start with `--show-timer`).
- Results are a single discreet line (`92 wpm · 98% acc · 95% con · 60.0s`).
- The timer now counts from the **first keystroke** (display fix).
- Removed the `figlet-rs` dependency; removed the `appearance.show_banner`/`figlet_font` config keys
  (config `schema_version` → 2; old keys are ignored on load).

## [0.1.0] - 2026-06-18

### Added — Phase 1 (MVP)
- Pure typing engine (`Action`/`Mode`/`TypingSession`) with grapheme/char-indexed state, space-skip,
  backspace and delete-word, and time injected as a parameter (deterministic, ghost-ready).
- Monkeytype-style metrics: net/raw WPM, accuracy, consistency, plus per-key error tallies.
- Challenge sources: embedded English/Spanish wordlists, and `import` from PDF (`pdf-extract`) and
  Word .docx (`zip`+`quick-xml`) through a shared text normalizer with a no-text-layer gate.
- Color themes (TOML) with 3 built-ins + 16-color fallback, and figlet ASCII banners.
- Config in the XDG dir with embedded defaults and no-panic fallback; editable test presets.
- Event-driven TUI (ratatui + crossterm): per-character colored typing view, live WPM/timer, results
  screen, RAII teardown + panic hook, gated kitty keyboard flags.
- CLI: default test, `--time/--words/--preset/--theme`, `import`, `config`, `theme`.
- 51 tests (unit + headless integration); CI runs fmt + clippy + test on Linux/macOS/Windows.

### Project setup
- Long-term-context docs (CLAUDE.md, docs/ARCHITECTURE, docs/ROADMAP, docs/PROGRESS), ADR-0001
  (tech stack), CI workflow, MIT license.

<!--
Phase tags:
  v0.1.0 = Phase 1 (MVP)
  v0.2.0 = Phase 2 (persistence + stats/charts)
  v0.3.0 = Phase 3 (ghost / shadow)
  v0.4.0 = Phase 4 (online)
-->
