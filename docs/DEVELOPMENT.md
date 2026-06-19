# Development, build & release

Operational notes so any session (or machine) can build, test and ship type-cli without re-deriving
the toolchain dance. For architecture see [`ARCHITECTURE.md`](ARCHITECTURE.md); for state see
[`PROGRESS.md`](PROGRESS.md).

## Prerequisites

- **Rust** (stable) via [rustup](https://rustup.rs). The repo pins `stable` in `rust-toolchain.toml`.
- On the dev box, `cargo` is **not** auto-added to PATH in non-interactive shells — prefix commands
  with `. "$HOME/.cargo/env"` (or source it once per shell).

## Everyday commands

```bash
. "$HOME/.cargo/env"
cargo run -- --time 60        # play (default: stealth, timer hidden, Ctrl+T to toggle)
cargo test                    # unit + headless integration tests
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```

CI parity (what must be green before a release): `cargo fmt --all -- --check`,
`cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, `cargo build --release`.

## Releasing a standalone Windows `.exe` (cross-compiled from Linux)

The bank/Windows machine usually **can't install the MSVC toolchain** (`link.exe`), so we ship a
prebuilt binary. We cross-compile to `x86_64-pc-windows-gnu` with **cargo-zigbuild** (uses `zig` as a
self-contained linker — no sudo, no mingw, no Visual Studio). The result depends only on **system
DLLs present on Windows 10/11** (verified with `objdump -p … | grep "DLL Name"`).

One-time toolchain setup (already done on the dev box):
```bash
# zig (self-contained tarball, no sudo) — lives at ~/zig-linux-x86_64-0.13.0 on this machine
curl -fsSL https://ziglang.org/download/0.13.0/zig-linux-x86_64-0.13.0.tar.xz -o /tmp/zig.tar.xz
tar -C "$HOME" -xf /tmp/zig.tar.xz
cargo install cargo-zigbuild
rustup target add x86_64-pc-windows-gnu
```

Build + publish a release:
```bash
. "$HOME/.cargo/env"
export PATH="$HOME/zig-linux-x86_64-0.13.0:$PATH"
cargo zigbuild --release --target x86_64-pc-windows-gnu
cp target/x86_64-pc-windows-gnu/release/type-cli.exe /tmp/type-cli-vX.Y.Z-windows-x86_64.exe
gh release create vX.Y.Z --repo jeisonxm/type-cli --latest \
  --title "vX.Y.Z — <name>" --notes-file <notes.md> /tmp/type-cli-vX.Y.Z-windows-x86_64.exe
```

Cutting a version: bump `version` in `Cargo.toml`, add a `## [X.Y.Z] - <date>` section to
`CHANGELOG.md`, commit, then tag via the `gh release create vX.Y.Z` above.

## Running on a corporate/locked Windows box (the bank PC)

If `cargo` fails with `CRYPT_E_NO_REVOCATION_CHECK` (schannel can't reach the cert-revocation server
through the proxy), disable the revocation check:
```powershell
$env:CARGO_HTTP_CHECK_REVOKE = "false"          # session-only
# or permanently: %USERPROFILE%\.cargo\config.toml  ->  [http]\ncheck-revoke = false
```
If building still fails (no MSVC `link.exe`), don't build — download the prebuilt `.exe` from the
GitHub release instead.

## CI status (action needed once)

`.github/workflows/ci.yml` exists on disk and is **intentionally untracked / not pushed**: the `gh`
OAuth token lacks the `workflow` scope, so GitHub rejects pushes that add workflow files. To enable
GitHub Actions CI:
```bash
gh auth refresh -s workflow -h github.com   # grant the scope (interactive, run via `! ...`)
git add .github/workflows/ci.yml && git commit -m "ci: enable GitHub Actions" && git push
```

## Repo & identity

- Public: **github.com/jeisonxm/type-cli**. `gh` is authenticated as `jeisonxm`.
- The local commit identity is set to the GitHub **noreply** email (privacy — no personal email in
  public history): `git config user.email` → `102503150+jeisonxm@users.noreply.github.com`.
