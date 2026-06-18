# ADR-0002 — Stealth UI (terminal-native camouflage)

- **Status:** Accepted
- **Date:** 2026-06-18
- **Deciders:** Jeison + agent
- **Supersedes:** the "figlet banners + fancy layout" part of ADR-0001.

## Context

After using the Phase 1 MVP, the real goal became clear: type-cli should let you **practice
touch-typing while looking like you're working in your normal terminal.** The MVP's design (large
figlet banners for the timer/WPM, a themed background fill, centered text, and a results "splash")
screams "game" — exactly what gives the practice away. "Like monkeytype" referred to the *behavior*
(a typing test), not the visual design.

## Decision

Make the UI **stealth-only** — it must be indistinguishable from plain terminal typing.

- **No background fill.** The terminal's own background shows through (`ui/mod.rs` renders no `Block`).
  The default theme `terminal` maps `bg`/`correct`/`caret` to `Color::Reset`.
- **No figlet, anywhere.** Removed `ui/banner.rs` and the `figlet-rs` dependency.
- **Typing screen** = plain text, top-left, left-aligned. Upcoming text is dim gray (reads like a
  zsh/fish autosuggestion); typed-correct uses the terminal's default fg; errors are red+underline;
  the caret is a reversed cell (like the terminal's block cursor). No hints/footer chrome.
- **Timer** is hidden by default and tiny/dim when shown; toggled at runtime with **Ctrl+T**
  (`appearance.show_timer` sets the initial state; `--show-timer` forces it on).
- **Results** = a single discreet line (`92 wpm · 98% acc · 95% con · 60.0s`), no splash.
- The **timer counts from the first keystroke**, not from when the screen opened (display fix; the
  engine already measured from the first keystroke).

## Alternatives considered

- **Keep a "fancy" mode as an option.** Rejected: the user chose stealth-only to keep the code and
  the UX focused; a second mode is dead weight for the mission.
- **Prose/code as the default text** (more convincing than random function words). Deferred: the user
  chose to keep random words for now. Revisit for stronger camouflage.

## Consequences

- Smaller, simpler render path and one fewer dependency (smaller binary).
- The `theme` system stays (customizes untyped/error colors) but `bg` is never painted.
- New input boundary: `input::Command { Engine(Action), ToggleTimer }` keeps UI hotkeys out of the
  pure engine.
- `appearance.show_banner`/`figlet_font` config keys are gone (`schema_version` bumped to 2; old keys
  are ignored on load, so existing configs keep working).
