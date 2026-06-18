# Roadmap

Four phases. Each builds on the last; the data model and module boundaries are already designed so
later phases are **additive** (no rewrites). For day-to-day state see [`PROGRESS.md`](PROGRESS.md).

---

## Phase 1 — MVP: a fully usable typing game (no database needed to play)

**Adds:** event loop, pure engine, type-along render, themes, figlet banners, config, random +
PDF/DOCX challenge sources, timed + word-count presets.

**Scope**
- Event-driven loop (`poll(timeout)+read()`, filter `KeyEventKind::Press`, dirty-flag redraw — no fixed 60 fps tick).
- RAII teardown + panic hook; kitty enhancement flags gated behind `supports_keyboard_enhancement()`.
- Pure engine: `Type / Backspace / DeleteWord / NextWord`, grapheme-indexed, pure metrics.
- Per-character colored render (cache, mutate only changed states); figlet timer/WPM; results screen.
- Config + ≥2 themes + a 16-color fallback, from the XDG config dir with embedded defaults.
- Sources: random wordlist + `import` PDF/DOCX → normalizer → passage.

**Acceptance criteria**
- `type-cli --time 60` runs a 60 s test end-to-end with live WPM/timer, correct/incorrect coloring,
  and a results screen (net + raw WPM, accuracy, consistency).
- `type-cli import file.pdf` and `file.docx` yield a typeable normalized passage; a scanned/no-text
  PDF shows a clear message.
- No terminal corruption on panic or quit (RAII teardown verified).
- Engine, stats and the normalizer are unit-tested and green.

---

## Phase 2 — Persistence, stats & charts

**Adds:** SQLite (`rusqlite` bundled) + migrations (`rusqlite_migration`), run history, charts,
most-failed-key detection, worst-word retry.

**Scope**
- Persist each run + `char_stat` + `worst_word` after a test; WAL pragmas; idempotent migrations.
- Results `Chart` (WPM line + accuracy/area), `BarChart` of most-failed keys.
- `type-cli stats`: WPM/accuracy over time, daily buckets, keyboard heatmap (colored Spans over QWERTY).
- "Retry worst words" → starts a new session from a run's worst words.

**Acceptance criteria**
- Runs persist across launches; `type-cli stats` renders history + most-failed key (min-sample gated).
- Retry-worst-words works; migrations run cleanly from an empty DB.

---

## Phase 3 — Ghost / shadow replay

**Adds:** keystroke-timeline capture + race-against-a-prior-run.

**Scope**
- Capture `(t_offset_ms, action)` (row table while prototyping → compressed BLOB in production).
- Ghost mode replays a prior run through the **same pure engine** (clock-as-parameter pays off);
  render the shadow cursor; record `ghost_of_run_id`.

**Acceptance criteria**
- "Race your best 60 s run" → the ghost advances on its recorded schedule; the result records which
  run was raced; replay is deterministic.

---

## Phase 4 — Online / multiplayer

**Adds:** identity sync + networked sessions.

**Scope**
- Populate `remote_id` / `synced_at`; upload unsynced runs.
- A network thread feeds events via `mpsc`; the loop `select`s over input + network using the same
  `Action` path. Backend protocol/auth/hosting decided in a dedicated P4 design spike (future ADR).

**Acceptance criteria**
- Local results upload; race vs a remote ghost / leaderboard; offline-first preserved.
- All additive — no schema rewrites.
