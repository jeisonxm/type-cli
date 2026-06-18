# type-cli

A fast, [monkeytype](https://monkeytype.com)-style **touch-typing game for the terminal**.
Type along with live per-character coloring, pick your color theme and figlet banner, and turn any
**PDF or Word document** into a typing challenge — all without leaving the terminal.

> Status: **Phase 1 (MVP) in progress.** See [`docs/ROADMAP.md`](docs/ROADMAP.md) and
> [`docs/PROGRESS.md`](docs/PROGRESS.md).

## Features

### Phase 1 — MVP (current)
- ⌨️  Monkeytype-style live typing: untyped / correct / incorrect / caret coloring per character.
- 🎨  Customizable **color themes** (TOML) and **figlet ASCII banners** for the timer, WPM and results.
- ⏱️  **Test modes**: timed (30 / 60 / 120 s) and word-count (100 / 500) — all editable in config.
- 📄  **Import challenges from PDF and Word (.docx)** — load a passage and type it.
- 📊  Live WPM + accuracy; results screen with net/raw WPM, accuracy and consistency.

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
cargo run -- --theme dracula           # pick a theme
cargo run -- import path/to/file.pdf   # type a document
cargo run -- import path/to/file.docx
```

Config and themes live in your XDG config dir (e.g. `~/.config/type-cli/`); the file is created on
first run and is fully editable.

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
