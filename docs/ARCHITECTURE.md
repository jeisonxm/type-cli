# Architecture

The "why" behind type-cli. For the stack decision and rejected alternatives see
[`adr/ADR-0001-tech-stack.md`](adr/ADR-0001-tech-stack.md).

## Guiding principles

1. **Latency is the product.** A typing game lives and dies on keystroke→paint latency, so the loop
   is event-driven (block on input, wake the timer ~10 Hz), never a fixed 60 fps tick.
2. **Pure core, impure shell.** All game logic (`engine`, `stats`, text `normalizer`) is pure and
   testable with zero terminal. The terminal, files and clock live at the edges (`main`, `input`,
   `ui`, `config`, `sources`, `storage`).
3. **Time is data.** The engine never reads the clock; callers pass `elapsed: Duration`. This makes
   metrics deterministic in tests and makes Phase 3 ghost-replay a trivial re-feed of a recorded timeline.

## Module map (`src/`)

```
main.rs        RAII teardown guard + panic hook; owns the event loop; wires everything.
cli.rs         clap parsing → an AppCommand (type / import / config / theme).
app.rs         App + AppState state machine (Menu → Typing → Results [→ Stats in P2]).
input.rs       crossterm KeyEvent → engine Action. The ONLY input-side crossterm code.
config.rs      XDG resolution (directories), load/merge config.toml, env override, first-run scaffold.

engine/        PURE. No ratatui/crossterm/I/O, no Instant::now().
  action.rs    Action enum: Type(char) / Backspace / DeleteWord / NextWord / Restart / Quit …
  presets.rs   Mode: Time{secs} | Words{count}; the editable preset list.
  session.rs   TypingSession: target, typed buffer, cursor, char_state(i), apply(Action, elapsed).

stats/         PURE. Metrics as functions of (history, elapsed).
  metrics.rs   wpm / raw_wpm / accuracy / consistency (monkeytype definitions).
  keystats.rs  per-key error tallies + worst words → most-failed key.

sources/       Challenge providers → a normalized passage.
  mod.rs       Source trait + the shared normalizer (NFC, de-hyphen, collapse, window, alpha gate).
  wordlist.rs  random words from an embedded list.
  pdf.rs       pdf-extract → text.
  docx.rs      zip + quick-xml → text.

ui/            RENDER only. Pure function of &App → Frame; never mutates state.
  typing_view.rs   per-character colored Paragraph.
  results_view.rs  figlet WPM + stat cards (+ run chart in P2).
  banner.rs        figlet timer/WPM/results banners.
  theme.rs         Theme struct (bg/untyped/correct/error/caret/accent…) → ratatui Color.

storage/       SQLite (Phase 2+). Kept out of the engine.
ghost.rs       Phase 3: replay a recorded timeline through the pure engine.
```

## Data flow (one keystroke)

```
crossterm Event::Key
  → input.rs maps to Action (filtering KeyEventKind::Press)
  → app.rs dispatches to engine.apply(action, elapsed)   [pure: mutates TypingSession]
  → loop marks dirty → terminal.draw(ui::render(&app))    [pure: state → Frame]
```

The same `Action` path is reused by tests (drive the engine directly), by Phase 3 ghost replay
(feed recorded actions with their `t_offset_ms`), and by Phase 4 networking (a network thread emits
`Action`s over an `mpsc` channel).

## Event loop (latency rationale)

```
loop {
    let timeout = next_tick.saturating_duration_since(now);   // ~100 ms cadence for the clock
    if event::poll(timeout)? {                                // wakes instantly on a keypress
        if let Event::Key(k) = event::read()? {
            if k.kind == KeyEventKind::Press { app.on_key(k); }
        }
    }
    if app.dirty() || tick_elapsed() { terminal.draw(|f| ui::render(f, &app))?; }
}
```

No separate input thread before Phase 4: `poll`/`read` already gives instant wakeup without a channel
hop. Render only on change; ratatui also diffs and writes only changed cells.

## Metrics (monkeytype definitions)

- `minutes = elapsed_ms / 60000`
- **Net WPM** = `(correct_chars / 5) / minutes`
- **Raw WPM** = `(typed_chars / 5) / minutes`
- **Accuracy** = `100 * correct_keystrokes / total_keystrokes`
- **Consistency** = `100 * (1 - stddev/mean)` of the per-second raw-WPM series, clamped to `[0, 100]`.
  The same series feeds the results chart.

## Data model (SQLite — implemented in Phase 2, designed now)

Immutable, append-only `test_run` fact table; aggregated `char_stat` / `worst_word` for cheap
analytics; `keystroke_event` (or a compressed `ghost_replay` BLOB) reserved for Phase 3;
`user.remote_id` / `test_run.remote_id` / `synced_at` reserved for Phase 4. All later columns are
nullable/additive, so no phase rewrites earlier schema. Pragmas: `journal_mode=WAL`,
`synchronous=NORMAL`, `foreign_keys=ON`. Migrations are inline Rust applied via `rusqlite_migration`
(state in `PRAGMA user_version`).

```sql
CREATE TABLE user (id INTEGER PRIMARY KEY, username TEXT NOT NULL,
    remote_id TEXT UNIQUE, created_at INTEGER NOT NULL);

CREATE TABLE preset (id INTEGER PRIMARY KEY, name TEXT NOT NULL,
    mode TEXT NOT NULL CHECK(mode IN ('time','words')),
    target INTEGER NOT NULL, is_builtin INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL);

CREATE TABLE test_run (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES user(id),
    preset_id INTEGER REFERENCES preset(id) ON DELETE SET NULL,
    mode TEXT NOT NULL CHECK(mode IN ('time','words')),
    target INTEGER NOT NULL,                                  -- denormalized snapshot
    source TEXT NOT NULL CHECK(source IN ('random','pdf','docx','quote','retry')),
    source_ref TEXT, language TEXT,
    wpm REAL NOT NULL, raw_wpm REAL NOT NULL,
    accuracy REAL NOT NULL, consistency REAL,
    chars_correct INTEGER NOT NULL DEFAULT 0,
    chars_incorrect INTEGER NOT NULL DEFAULT 0,
    chars_extra INTEGER NOT NULL DEFAULT 0,
    chars_missed INTEGER NOT NULL DEFAULT 0,
    elapsed_ms INTEGER NOT NULL,
    ghost_of_run_id INTEGER REFERENCES test_run(id) ON DELETE SET NULL,   -- P3
    remote_id TEXT UNIQUE, synced_at INTEGER,                             -- P4
    started_at INTEGER NOT NULL,                              -- unix epoch ms, timeline key
    created_at INTEGER NOT NULL);

CREATE TABLE char_stat (id INTEGER PRIMARY KEY,
    run_id INTEGER NOT NULL REFERENCES test_run(id) ON DELETE CASCADE,
    expected_char TEXT NOT NULL, typed_total INTEGER NOT NULL DEFAULT 0,
    error_count INTEGER NOT NULL DEFAULT 0, UNIQUE(run_id, expected_char));

CREATE TABLE worst_word (id INTEGER PRIMARY KEY,
    run_id INTEGER NOT NULL REFERENCES test_run(id) ON DELETE CASCADE,
    word TEXT NOT NULL, error_count INTEGER NOT NULL DEFAULT 0,
    word_wpm REAL, rank INTEGER NOT NULL);

CREATE TABLE keystroke_event (id INTEGER PRIMARY KEY,         -- P3 ghost (row form, prototyping)
    run_id INTEGER NOT NULL REFERENCES test_run(id) ON DELETE CASCADE,
    t_offset_ms INTEGER NOT NULL, char_index INTEGER NOT NULL,
    typed_char TEXT, is_correct INTEGER NOT NULL DEFAULT 0,
    is_backspace INTEGER NOT NULL DEFAULT 0);

CREATE INDEX idx_run_user_time ON test_run(user_id, started_at);
CREATE INDEX idx_run_user_mode ON test_run(user_id, mode, target, started_at);
CREATE INDEX idx_charstat_run  ON char_stat(run_id);
CREATE INDEX idx_worstword_run ON worst_word(run_id, rank);
CREATE INDEX idx_run_unsynced  ON test_run(synced_at) WHERE synced_at IS NULL;  -- P4
```

Key queries (full list lives next to the code in Phase 2): WPM/accuracy over time, daily buckets,
most-failed key (with a `HAVING SUM(typed_total) >= 20` min-sample gate), retry worst words of a run.
