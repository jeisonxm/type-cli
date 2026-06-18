# type-cli

A fast, [monkeytype](https://monkeytype.com)-style **touch-typing trainer for the terminal** — but
**stealth**: it looks like you're just typing in your normal terminal while you practice. No banners,
no background, no chrome; upcoming text is dimmed like a shell autosuggestion. Turn any **PDF or Word
document** into a challenge, too.

> Status: **Phase 1 (MVP) + stealth UI done.** See [`docs/ROADMAP.md`](docs/ROADMAP.md) and
> [`docs/PROGRESS.md`](docs/PROGRESS.md).

## Features

### Phase 1 — MVP (current)
- 🥷  **Stealth look**: plain top-left text that blends with your terminal (dim upcoming text, normal
  typed text, red errors). The timer is **hidden by default** — press **Ctrl+T** to peek at it.
- ⌨️  Live per-character feedback (correct / incorrect / caret) as you type.
- ⏱️  **Test modes**: timed (30 / 60 / 120 s) and word-count (100 / 500) — all editable in config.
- 📄  **Import challenges from PDF and Word (.docx)** — load a passage and type it.
- 📊  One-line results: `92 wpm · 98% acc · 95% con · 60.0s`. The timer starts on your first keystroke.

### Coming next
- **Phase 2** — persistent records (SQLite), progress charts (WPM/accuracy over time), most-failed
  key detection, and "retry the worst words" of a test.
- **Phase 3** — race your own *ghost*: replay a previous run and beat your shadow.
- **Phase 4** — compete online.

## Install & run

Requires the [Rust toolchain](https://rustup.rs).

```bash
git clone <repo> && cd type-cli
cargo run -- --time 60                 # 60-second test
cargo run -- --words 100               # 100-word test
cargo run -- --show-timer              # start with the timer visible
cargo run -- import path/to/file.pdf   # type a document
cargo run -- import path/to/file.docx
```

In-game: type along · **Ctrl+T** show/hide the timer · **Tab** restart · **Esc** quit.

Config and themes live in your XDG config dir (e.g. `~/.config/type-cli/`); the file is created on
first run and is fully editable. Prefer some color? `--theme serika_dark|dracula|classic16`.

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```

Architecture and contribution guidelines: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md),
[`CONTRIBUTING.md`](CONTRIBUTING.md), [`docs/adr/`](docs/adr/).

## License

MIT — see [`LICENSE-MIT`](LICENSE-MIT).
