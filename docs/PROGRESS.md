# PROGRESS — the living state of type-cli

> Read this first. Update it last (see the ritual in `CLAUDE.md`).
> Three buckets: **lo que se crea (Done) / lo que hace falta (Missing) / en lo que vamos (Now)**.

_Last updated: 2026-06-18 — Phase 1 MVP + stealth UI redesign complete._

---

## Now (en lo que vamos)

- **Phase:** 1 (MVP) ✅ + **stealth UI** ✅. Next up: Phase 2 (persistence + stats/charts).
- **In flight:** nothing. Ready to start P2 (SQLite via `rusqlite` bundled + `rusqlite_migration`).
- **Last decision:** **stealth-only UI** (ADR-0002) — the mission is to look like normal terminal work
  while practicing. No figlet/background/chrome; timer hidden + Ctrl+T toggle; one-line results.
- **Blockers:** none.
- **How to play:** `cargo run -- --time 60` · `--words 100` · `--show-timer` · `import file.pdf`.
  In-game: type along · `Ctrl+T` show/hide timer · `Tab` restart · `Esc` quit.

---

## Done (lo que se crea)

- **2026-06-18** — **Stealth UI redesign** (ADR-0002): removed figlet (`ui/banner.rs` + `figlet-rs`
  dep) and the background fill; typing screen is plain top-left text (dim upcoming, reset correct, red
  errors, reversed caret); `terminal` theme (`Color::Reset`) is the default; timer hidden by default
  with `Ctrl+T` toggle (`input::Command` enum); one-line results; timer counts from the first
  keystroke (display fix). 53 tests green; verified headless + real-tty pty.
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

### Phase 2 — Persistence, stats & charts (next)
1. Add `rusqlite` (bundled) + `rusqlite_migration`; create `storage/` with the schema in
   `docs/ARCHITECTURE.md` (migration 1 seeds a `local` user). _Acceptance: migrations idempotent from empty DB._
2. Persist each finished run + `char_stat` + `worst_word`. _Acceptance: runs survive across launches._
3. `type-cli stats`: WPM/accuracy-over-time `Chart`, daily buckets, `BarChart` of most-failed keys,
   keyboard heatmap (colored Spans over QWERTY). _Acceptance: history + most-failed key render (min-sample gated)._
4. "Retry worst words" → start a session from a run's worst words. _Acceptance: retry flow works._

### Phase 3 — Ghost / shadow replay
- Capture keystroke timeline; race a prior run replayed through the same pure engine. (ADR-0004: row vs BLOB.)

### Phase 4 — Online / multiplayer
- Identity sync (`remote_id`/`synced_at`); network thread feeds `Action`s via `mpsc`. (ADR-0005: protocol.)

### Polish / known gaps (any time)
- Long-passage rendering uses a fitted window instead of a true scroll widget (good enough; revisit if needed).
- Strict vs lenient accuracy for skipped letters (see "Last decision").
- Bundle extra `.flf` figlet fonts + honor `figlet_font` beyond "standard".

(Full acceptance criteria per phase: `docs/ROADMAP.md`.)
