# Changelog

All notable changes to this project are documented here.
Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
