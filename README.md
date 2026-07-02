# sync-ai-clis

<p align="center">
  <a href="https://github.com/hyeonbungi/sync-ai-clis/actions/workflows/ci.yml">
    <img alt="CI" src="https://img.shields.io/github/actions/workflow/status/hyeonbungi/sync-ai-clis/ci.yml?branch=main&style=flat-square&label=CI">
  </a>
  <a href="https://github.com/hyeonbungi/sync-ai-clis/releases/latest">
    <img alt="GitHub release" src="https://img.shields.io/github/v/release/hyeonbungi/sync-ai-clis?style=flat-square&label=release&color=2f80ed">
  </a>
  <img alt="Platforms: macOS, Windows, Linux" src="https://img.shields.io/badge/platforms-macOS%20%C2%B7%20Windows%20%C2%B7%20Linux-44cc11?style=flat-square">
  <a href="./LICENSE">
    <img alt="License: MIT" src="https://img.shields.io/badge/license-MIT-111827?style=flat-square">
  </a>
</p>

<p align="center">
  <a href="https://crates.io/crates/sync-ai-clis">
    <img alt="crates.io" src="https://img.shields.io/crates/v/sync-ai-clis?style=flat-square&logo=rust&logoColor=white&label=crates.io&color=2f80ed">
  </a>
  <a href="https://www.npmjs.com/package/sync-ai-clis">
    <img alt="npm" src="https://img.shields.io/npm/v/sync-ai-clis?style=flat-square&label=npm&color=2f80ed">
  </a>
  <a href="https://github.com/hyeonbungi/scoop-bucket">
    <img alt="Scoop" src="https://img.shields.io/scoop/v/sync-ai-clis?bucket=https%3A%2F%2Fgithub.com%2Fhyeonbungi%2Fscoop-bucket&style=flat-square&label=scoop&color=2f80ed">
  </a>
  <a href="https://github.com/hyeonbungi/sync-ai-clis/pkgs/container/sync-ai-clis">
    <img alt="ghcr.io container" src="https://img.shields.io/badge/ghcr.io-container-2f80ed?style=flat-square&logo=docker&logoColor=white">
  </a>
</p>

<p align="center">
  English | <a href="./README.ko.md">한국어</a>
</p>

> One command to detect, install, and keep your AI coding CLIs up to date — Claude Code, Codex, Gemini, Kiro, and Antigravity.

<p align="center">
  <img alt="sync-ai-clis --dry-run: every tool, its detected install channel, and the exact commands that would run" src="https://raw.githubusercontent.com/hyeonbungi/sync-ai-clis/main/.github/assets/terminal-demo.svg" width="600">
</p>

`sync-ai-clis` is a cross-platform (macOS · Windows · Linux) Rust CLI that reconciles your machine toward one state: every known AI CLI installed, working, and current. Installed tools get updated. Missing tools are installed after you consent. Then every tool is re-verified by actually running `--version`, which catches broken installs that `command -v` would miss.

**Current status: released.** `list`, `doctor`, `check`, `audit`, `--dry-run`, and consent-based install/update all work, verified by 150 tests plus real-channel runs on Linux containers, macOS, and Windows CI. [SPEC.md](./SPEC.md) holds the full design — confirmed decisions, architecture, the per-tool install/update matrix, and the test and release strategy. It is the single source of truth for this repository.

## At A Glance

| Area | Current Value |
| --- | --- |
| Purpose | Detect · install (with consent) · update · verify AI coding CLIs |
| Managed tools (v1) | `claude`, `codex`, `gemini`, `kiro-cli`, `agy` |
| Platforms | macOS · Windows · Linux |
| Stack | Rust (single binary) |
| Status | Released — engine verified on all three OSes (150 offline tests + real-channel CI) |
| Distribution | GitHub Releases · Homebrew tap · npm · crates.io · winget · Scoop · ghcr (Docker) |
| Tests | 150 offline tests + Docker distro matrix + 3-OS CI with real-channel runs |
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

```dockerfile
# Docker / devcontainers — bake AI CLIs into your image (static musl binary, amd64+arm64)
COPY --from=ghcr.io/hyeonbungi/sync-ai-clis:latest /sync-ai-clis /usr/local/bin/sync-ai-clis
RUN sync-ai-clis --yes --only claude,gemini
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
sync-ai-clis doctor          # read-only diagnosis: broken, duplicate, or shadowed installs
sync-ai-clis check           # read-only: is an update available? signals via exit code (CI/cron/prompt badge)
sync-ai-clis audit           # read-only: did a remote install script change? unified diff, --accept to re-baseline
sync-ai-clis --json          # machine-readable summary
```

`--only`, `--except`, and `--json` are global flags, so they also work after subcommands, for example `sync-ai-clis doctor --only gemini --json`.

Exit codes: `0` all OK · `1` any failure · `2` usage error (`check` and `audit` add `10` = an update is available / a script changed). Configuration lives in `~/.config/sync-ai-clis/config.toml`, and flags win over config.

## Trust Model

This tool runs remote official installers (`curl | bash`, `irm | iex`) and package-manager commands, so its security rules are spelled out rather than left implicit ([SPEC.md](./SPEC.md) §5.5):

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
cargo test                 # 150 offline tests — no network, no system changes
cargo fmt --check && cargo clippy --all-targets -- -D warnings
cargo run -- list          # read-only: detect tools and show versions
cargo run -- --dry-run     # show exactly what a sync would run, execute nothing
docker/run-matrix.sh       # real install/update integration (disposable containers only)
```

For the reasoning behind it — decisions, architecture, the per-tool command matrix, and the test strategy — see [SPEC.md](./SPEC.md). Real installs and updates never run on a development machine; that is what the Docker matrix and CI runners are for.

## Known Limitations

- **Kiro on Windows**: needs Windows 11. Command selection is covered offline; full desktop install verification belongs in Windows CI or a disposable VM, not on a development machine.
- **Alpine/musl**: the sync-ai-clis binary runs on musl, but most upstream installers do not ship musl builds yet.
- **Config `[channels]` overrides** apply to update planning only. Missing-tool installs and `doctor` diagnostics still use the real detected state.

## Maintenance Signals

- Contribution guide: [CONTRIBUTING.md](./CONTRIBUTING.md)
- Security policy and trust model: [SECURITY.md](./SECURITY.md)
- Change log: [CHANGELOG.md](./CHANGELOG.md)
- CI: 3-OS tests on every push, weekly real-channel integration

## Origin

This tool grew out of a personal macOS-only bash script (`update-ai-clis`) that updated and re-verified five AI CLIs. v1 generalizes it to three OSes, with consent-based installation of missing tools and public distribution channels.

## Author

- [hyeonbungi](https://github.com/hyeonbungi) (김현우)

## License

MIT. See [LICENSE](./LICENSE).
