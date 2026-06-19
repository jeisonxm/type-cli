# PROGRESS — the living state of type-cli

> Read this first. Update it last (see the ritual in `CLAUDE.md`).
> Three buckets: **lo que se crea (Done) / lo que hace falta (Missing) / en lo que vamos (Now)**.

_Last updated: 2026-06-19 — Phase 2 complete: PR1 (persistence) + PR2 (stats TUI, retry, sparkline).
`type-cli stats` renders history + most-missed keys + QWERTY heatmap; "retry worst words" works._

---

## Now (en lo que vamos)

- **Phase:** 2 (persistence + stats/charts) — **PR1 ✅ + PR2 ✅ done.** Phase 2 feature-complete.
  Next: Phase 3 (ghost/shadow replay).
- **In flight:** nothing. PR1 is committed on `feat/phase2-persistence`; PR2 is committed on
  `feat/phase2-stats` (stacked on PR1). Neither branch is pushed / PR-opened yet.
- **Decision (P2, ADR-0003):** stats is a **full TUI** (Chart + BarChart + QWERTY heatmap), an
  *opt-in* exception to stealth (only on `type-cli stats`); results screen gained a discreet WPM/sec
  **sparkline** gated behind the timer toggle. Typing screen unchanged (still stealth).
- **Prior decision:** **stealth-only UI** (ADR-0002) — look like normal terminal work while
  practicing. No figlet/background/chrome; timer hidden + Ctrl+T toggle; one-line results.
- **Blockers:** none.
- **How to play:** `cargo run -- --time 60` · `--words 100` · `--show-timer` · `import file.pdf` ·
  `stats`. In-game: type along · `Ctrl+T` show/hide timer · `Tab` restart · `Esc` quit.
  In stats: `r` retry worst words · `q`/`Esc` quit.
- **Build/release/ops:** see [`DEVELOPMENT.md`](DEVELOPMENT.md). Public repo:
  github.com/jeisonxm/type-cli · latest release **v0.1.3** (standalone Windows `.exe`).
  NOTE: `.github/workflows/ci.yml` exists on disk but is **not pushed** — the `gh` token lacks the
  `workflow` scope. To enable CI: `gh auth refresh -s workflow -h github.com`, then track & push it.

---

## Done (lo que se crea)

- **2026-06-19** — **Phase 2 · PR2: stats TUI + retry + sparkline (branch `feat/phase2-stats`).**
  `storage/queries.rs`: `recent_runs`, `run_count`, `key_aggregates` (min-sample `HAVING` gate),
  `most_recent_worst_words`. New `stats_app::StatsApp` (loads the queries; `q`/`Esc` quit, `r` →
  retry). New `ui/stats_view.rs` (pure render): WPM `Chart`, `BarChart` of most-missed keys, colour
  QWERTY heatmap, empty-state. `type-cli stats` subcommand + `run_stats`/`stats_loop` in `main.rs`
  (reuses the RAII teardown). `SourceKind::Retry(Vec<String>)` seeds a drill from the worst words
  (persisted as `source='retry'`). Results screen gained a discreet WPM/sec `Sparkline`, gated by
  the timer toggle. **ADR-0003** records the opt-in stealth exception. 74 tests green (headless stats
  render with seeded DB, empty-state, retry round-trip), clippy/fmt clean. **Not yet pushed.**
- **2026-06-19** — **Phase 2 · PR1: persistence (branch `feat/phase2-persistence`).** SQLite via
  `rusqlite` (bundled) + `rusqlite_migration`. New `storage/` shell module (kept out of the pure
  core): `Store::open` applies WAL/NORMAL/foreign_keys pragmas + runs migration 1 (full schema from
  `ARCHITECTURE.md` + seeded `local` user); `insert_run` writes `test_run` + `char_stat` +
  `worst_word` in one transaction. Pure stats extended: `Summary` gains `missed_chars`/`extra_chars`;
  `stats::keystats::worst_words(session)` ranks mistyped words with per-word WPM. `App` opens the DB
  (best-effort; failure → persistence off, never a crash) and persists in `maybe_finish`. 64 tests
  green (incl. e2e: a finished run survives a relaunch), clippy/fmt clean. **Not yet committed.**
- **2026-06-19** — **v0.1.3: error styling is colour-only (dropped `UNDERLINED`).** The underline
  modifier made ratatui emit `ESC[4m` on every error cell; removing it lowers the escape surface on
  minimal consoles and is more stealth. Verified via a real-pty byte audit: `ESC[4m` 0 (was 2412),
  no parser desync. Note: this is hardening — the "frozen terminal after a mistype" was v0.1.2's
  cursor-restore fix; the `ESC[…m` reset tail in the leak is ratatui's normal per-frame reset, not the
  underline. Guarded by a new test (`mistyped_chars_are_never_underlined`). 55 tests green.
- **2026-06-19** — **Bugfix v0.1.2: terminal cursor left hidden on exit.** ratatui hides the cursor
  every frame and the teardown never re-showed it, so after a mistype the terminal looked "frozen"
  (the leak ended in `␛[?25l`). Teardown now restores the cursor (`Show`) on both the RAII-drop and
  panic-hook paths, factored into a unit-tested `restore_terminal(out, kitty)` helper (`src/main.rs`).
  54 tests green (added a teardown regression test — the contract no headless test had covered).
- **2026-06-18** — **Stealth UI redesign** (ADR-0002): removed figlet (`ui/banner.rs` + `figlet-rs`
  dep) and the background fill; typing screen is plain top-left text (dim upcoming, reset correct, red
  errors, reversed caret); `terminal` theme (`Color::Reset`) is the default; timer hidden by default
  with `Ctrl+T` toggle (`input::Command` enum); one-line results; timer counts from the first
  keystroke (display fix). 53 tests green; verified headless + real-tty pty. **Shipped as Release
  v0.1.1** (standalone stealth Windows `.exe`, cross-compiled with zigbuild).
- **2026-06-18** — Cross-compiled a standalone Windows `.exe` (zigbuild → windows-gnu) and published
  GitHub Release `v0.1.0`. Repo public at github.com/jeisonxm/type-cli.
- **2026-06-18** — Repo scaffolding: `Cargo.toml`, `.gitignore`, `rust-toolchain.toml`, CI workflow,
  MIT license, `CLAUDE.md`, `README`, `CONTRIBUTING`, `CHANGELOG`, `docs/{ARCHITECTURE,ROADMAP,PROGRESS}`,
  `adr/ADR-0001`. Rust 1.96 installed.
- **2026-06-18** — **Phase 1 (MVP) shipped.** 51 tests green; `fmt` + `clippy -D warnings` clean.
  - `engine/` (pure): `Action`, `Mode`, `TypingSession` (grapheme/char-indexed, space-skip, backspace,
    delete-word, timer-as-parameter). 11 unit tests.
  - `stats/` (pure): net/raw WPM, accuracy, consistency, per-second series, `Summary`, per-key tallies. 9 tests.
  - `sources/`: normalizer (NFC, smart-punct fold, de-hyphen, collapse, alpha-ratio gate), embedded
    wordlists (en/es), PDF (`pdf-extract`), DOCX (`zip`+`quick-xml`, real round-trip test). 15 tests.
  - `config.rs` + `ui/theme.rs`: XDG dirs, `config.toml` + themes with embedded defaults and no-panic
    fallback; 3 built-in themes (serika_dark, dracula, classic16). 7 tests.
  - `ui/` + `input.rs` + `app.rs` + `main.rs`: event-driven loop (`poll`+`read`, `KeyEventKind::Press`,
    RAII teardown + panic hook, gated kitty flags), per-character colored typing view, figlet banners,
    results screen. 5 tests + 3 headless integration tests (`tests/app_session.rs`).
  - `cli.rs`: `type-cli` (default preset), `--time/--words/--preset/--theme`, `import <file>`, `config`, `theme`.
  - Verified: headless render via `TestBackend`; real-tty pty smoke test (renders a full frame,
    responds to keys, exits cleanly with the terminal restored).

---

## Missing (lo que hace falta) — ordered

### Phase 2 — Persistence, stats & charts ✅ done
1. ✅ **Done (PR1).** `rusqlite` (bundled) + `rusqlite_migration`; `storage/` with the schema from
   `docs/ARCHITECTURE.md` (migration 1 seeds a `local` user). Migrations idempotent from empty DB.
2. ✅ **Done (PR1).** Each finished run + `char_stat` + `worst_word` is persisted; runs survive
   across launches (e2e test).
3. ✅ **Done (PR2).** `type-cli stats` — WPM-over-time `Chart`, `BarChart` of most-missed keys,
   QWERTY heatmap (min-sample gated), plus a timer-gated WPM/sec sparkline on the results screen.
   Recorded in **ADR-0003**. _(Daily-bucket view not built — the per-run timeline covers the need;
   revisit if wanted.)_
4. ✅ **Done (PR2).** "Retry worst words" → `SourceKind::Retry` seeds a session from the latest
   run's worst words (launched with `r` from the stats screen).

### Phase 3 — Ghost / shadow replay
- Capture keystroke timeline; race a prior run replayed through the same pure engine. (ADR-0004: row vs BLOB.)

### Phase 4 — Online / multiplayer
- Identity sync (`remote_id`/`synced_at`); network thread feeds `Action`s via `mpsc`. (ADR-0005: protocol.)

### Polish / known gaps (any time)
- Long-passage rendering uses a fitted window instead of a true scroll widget (good enough; revisit if needed).
- Strict vs lenient accuracy for skipped letters (see "Last decision").
- Stronger camouflage: optional prose/realistic text source (random function-words read like a list).
- Enable CI on GitHub once the `workflow` scope is granted (see Now → Build/release/ops).

(Full acceptance criteria per phase: `docs/ROADMAP.md`.)
