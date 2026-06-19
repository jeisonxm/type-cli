# Changelog

All notable changes to this project are documented here.
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
