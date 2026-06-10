# sync-ai-clis

<p align="center">
  <a href="https://crates.io/crates/sync-ai-clis">
    <img alt="crates.io" src="https://img.shields.io/crates/v/sync-ai-clis?style=flat-square&color=2f80ed">
  </a>
  <img alt="Stack: Rust" src="https://img.shields.io/badge/stack-Rust-dea584?style=flat-square">
  <img alt="Platforms: macOS, Windows, Linux" src="https://img.shields.io/badge/platforms-macOS%20%C2%B7%20Windows%20%C2%B7%20Linux-44cc11?style=flat-square">
  <a href="https://github.com/hyeonbungi/sync-ai-clis/actions/workflows/ci.yml">
    <img alt="CI" src="https://github.com/hyeonbungi/sync-ai-clis/actions/workflows/ci.yml/badge.svg">
  </a>
  <a href="./LICENSE">
    <img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-111827?style=flat-square">
  </a>
</p>

<p align="center">
  English | <a href="./README.ko.md">한국어</a>
</p>

> One command to detect, install, and keep your AI coding CLIs up to date — Claude Code, Codex, Gemini, Kiro, and Antigravity. "rustup, but for AI CLIs."

`sync-ai-clis` is a cross-platform (macOS · Windows · Linux) Rust CLI that reconciles your machine toward "every known AI CLI installed, working, and current": installed tools get updated, missing tools are installed after consent, and each tool is re-verified after the work (`--version` must actually run, catching broken installs — not just `command -v`).

**Current status: released.** `list`, `--dry-run`, and consent-based install/update all work, verified by 84 tests plus real-channel runs on Linux containers, macOS, and Windows CI. The full design — confirmed decisions, architecture, per-tool install/update matrix, test and release strategy — lives in [SPEC.md](./SPEC.md), the single source of truth for this repository.

## At A Glance

| Area | Current Value |
| --- | --- |
| Purpose | Detect · install (with consent) · update · verify AI coding CLIs |
| Managed tools (v1) | `claude`, `codex`, `gemini`, `kiro-cli`, `agy` |
| Platforms | macOS · Windows · Linux |
| Stack | Rust (single binary) |
| Status | Released — engine verified on all three OSes (84 offline tests + real-channel CI) |
| Distribution | GitHub Releases · Homebrew tap · npm · crates.io · winget · Scoop |
| Tests | 84 offline tests + Docker distro matrix + 3-OS CI with real-channel runs |
| License | [MIT](./LICENSE) |
| Author | [hyeonbungi](https://github.com/hyeonbungi) |

## Install

```sh
# Homebrew (macOS · Linux)
brew install hyeonbungi/tap/sync-ai-clis

# Shell installer (macOS · Linux)
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/hyeonbungi/sync-ai-clis/releases/latest/download/sync-ai-clis-installer.sh | sh

# npm
npm install -g sync-ai-clis

# cargo
cargo install sync-ai-clis
```

```powershell
# winget (Windows)
winget install hyeonbungi.sync-ai-clis

# Scoop (Windows)
scoop bucket add hyeonbungi https://github.com/hyeonbungi/scoop-bucket
scoop install sync-ai-clis

# PowerShell installer (Windows)
powershell -ExecutionPolicy Bypass -c "irm https://github.com/hyeonbungi/sync-ai-clis/releases/latest/download/sync-ai-clis-installer.ps1 | iex"
```

## Usage

The contract from [SPEC.md](./SPEC.md) §6, fully implemented:

```text
sync-ai-clis                 # default: update installed tools, ask (y/N) before installing missing ones
sync-ai-clis --yes, -y       # non-interactive: install missing + update everything (CI-friendly)
sync-ai-clis --no-install    # update only, never offer to install
sync-ai-clis --only claude,gemini
sync-ai-clis --except kiro
sync-ai-clis --dry-run       # print the exact commands, execute nothing
sync-ai-clis list            # known tools + installed/current version table (alias: status)
sync-ai-clis --json          # machine-readable summary
```

Exit codes: `0` all OK · `1` any failure · `2` usage error. Configuration lives in `~/.config/sync-ai-clis/config.toml` (flags win over config).

## Trust Model

This tool executes remote official installers (`curl | bash`, `irm | iex`) and package-manager commands, so its security rules are explicit by design ([SPEC.md](./SPEC.md) §5.5):

- Install/update URLs are **hardcoded official HTTPS constants** in the tool registry — neither config nor flags can inject arbitrary URLs.
- Installing a missing tool requires **consent**: an interactive prompt or an explicit `--yes`.
- `--dry-run` prints **exactly** the commands that would run.
- **No automatic privilege escalation** — the tool never runs sudo/UAC elevation on its own.

## Repository Map

| Path | Purpose |
| --- | --- |
| `SPEC.md` | Design source of truth: decisions, architecture, tool matrix, test strategy |
| `Cargo.toml`, `src/` | Rust crate — engine, tool registry, CLI (see `SPEC.md` §5) |
| `tests/` | Integration tests (OS × state command-selection matrix, binary smoke) |
| `docker/` | Linux integration matrix — the only place real installs run locally |
| `.github/workflows/` | 3-OS CI, real-channel integration, release pipeline, winget publish |
| `dist-workspace.toml` | Release and packaging configuration ([dist](https://github.com/axodotdev/cargo-dist)) |

## Development

```bash
cargo test                 # 84 offline tests — no network, no system changes
cargo fmt --check && cargo clippy --all-targets -- -D warnings
cargo run -- list          # read-only: detect tools and show versions
cargo run -- --dry-run     # show exactly what a sync would run, execute nothing
docker/run-matrix.sh       # real install/update integration (disposable containers only)
```

Design rationale — decisions, architecture, the per-tool command matrix, and the test strategy — lives in [SPEC.md](./SPEC.md). Real installs and updates are never exercised on a development machine: that is what the Docker matrix and CI runners are for.

## Known Limitations

- **Kiro on Windows**: requires Windows 11, and the exact official install command is not confirmed upstream yet — sync-ai-clis reports a clear SKIP instead of guessing a URL (already-installed `kiro-cli` still self-updates fine). Tracked in [SPEC.md](./SPEC.md) §11.
- **Alpine/musl**: the sync-ai-clis binary itself runs on musl, but most upstream installers do not ship musl builds yet.
- **Config `[channels]` overrides** are parsed but not applied to channel selection yet.

## Maintenance Signals

- Contribution guide: [CONTRIBUTING.md](./CONTRIBUTING.md)
- Security policy and trust model: [SECURITY.md](./SECURITY.md)
- Change log: [CHANGELOG.md](./CHANGELOG.md)
- CI: 3-OS tests on every push, weekly real-channel integration

## Origin

This tool grows out of a personal macOS-only bash script (`update-ai-clis`) that updated and re-verified five AI CLIs. v1 generalizes it: three OSes, consent-based installation of missing tools, and public distribution channels.

## Author

- [hyeonbungi](https://github.com/hyeonbungi) (김현우)

## License

MIT. See [LICENSE](./LICENSE).
