# ADR-0003 — Stats visualization (opt-in exception to stealth)

- **Status:** Accepted
- **Date:** 2026-06-19
- **Deciders:** Jeison + agent
- **Relates to:** ADR-0002 (Stealth UI). This is a *bounded exception*, not a reversal.

## Context

Phase 2 adds persistence (ADR-0001's SQLite layer) and the analytics on top: a typing-history
chart, most-missed keys, and a keyboard heatmap. ADR-0002 made the UI **stealth-only** — the typing
and results screens must look like ordinary terminal work. A colourful chart + heatmap is the
opposite of stealth, so we need an explicit decision on where rich visuals are allowed.

## Decision

Allow rich, deliberately-visual analytics, but only **opt-in** — never on screen during the act of
practicing.

- **`type-cli stats` is a full TUI** (ratatui `Chart` for WPM-over-time, `BarChart` of most-missed
  keys, a colour-coded QWERTY heatmap). It renders only when the user explicitly runs that
  subcommand, so it can never "out" the practice mid-session. Lives in `ui/stats_view.rs`
  (pure render) driven by `stats_app::StatsApp` (loads the DB queries).
- **The results screen stays stealth by default.** It gains a small WPM/sec `Sparkline`, but that is
  **gated behind the timer toggle** (`show_timer` / Ctrl+T) — hidden by default, exactly like the
  timer. With the timer off, results remain the single discreet line from ADR-0002.
- **The typing screen is untouched** — still plain, top-left text with no chrome.

## Alternatives considered

- **Keep stats text-only (stealth everywhere).** Rejected by the user: the analytics are reviewed
  intentionally, away from prying eyes, so a readable chart/heatmap is worth more than uniformity.
- **Always-on sparkline / chart on the results screen.** Rejected: results can appear right after a
  test while someone is watching; tying the sparkline to the existing timer gate keeps the stealth
  default and reuses a control the user already knows.

## Consequences

- `ui/` gains a visual module (`stats_view`) that, unlike the rest of the layer, paints blocks,
  axes, bars and colours. The stealth invariant now reads: *the typing and (default) results screens
  are stealth; explicit analytics views may be visual.*
- The stealth-by-default contract is preserved end-to-end: nothing rich shows unless the user either
  runs `type-cli stats` or has already toggled the timer on.
- Retry-worst-words is launched from the stats screen (`r`), flowing back into the normal stealth
  typing TUI via `SourceKind::Retry`.
