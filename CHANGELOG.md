# Changelog

All notable changes to `sync-ai-clis` are tracked here.

## 0.4.2 - 2026-06-20

### Fixed

- The Windows `Get-FileHash` fallback now accepts `-LiteralPath` as well as `-Path`. Codex's official `install.ps1` verifies its downloaded archive with `Get-FileHash -LiteralPath ...`, so 0.4.1 could still fail in PowerShell environments where `Microsoft.PowerShell.Utility` was unavailable.

## 0.4.1 - 2026-06-20

### Fixed

- Windows native installers now bootstrap a local `Get-FileHash` fallback before running official `install.ps1` scripts. This keeps Claude Code, Codex, Kiro, and Antigravity installs working in PowerShell environments where `Microsoft.PowerShell.Utility` is unavailable, while leaving the hardcoded official HTTPS installer URLs unchanged.

## 0.4.0 - 2026-06-17

### Added

- `audit` subcommand: read-only detection of changes in remote install scripts. It fetches each tool's install script (`curl|bash` / `irm|iex` for Claude Code, Codex, Kiro, and Antigravity; Gemini installs through npm, so it is not applicable), compares it against the last accepted baseline kept under your data directory, and reports `unchanged` / `changed` (with a unified diff) / `unregistered` / `unavailable` / `not-applicable` — without changing anything. Plain `audit` never writes to disk; `audit --accept` records the current scripts as the new trusted baseline (the only write path). Exit code `10` when any script changed, `1` when a fetch was inconclusive, `0` otherwise; `--json` is supported. It is the fourth pillar of the trust model (SPEC §5.5): a practical defense against a `curl | bash` install script changing out from under you between runs.

## 0.3.0 - 2026-06-16

### Added

- `check` subcommand: read-only update-availability per tool. It probes each installed tool's `--version` and its latest available version (the npm registry for Claude Code, Codex, and Gemini; the official release manifest for Antigravity), compares them, and reports `current` / `update-available` / `unknown` / `not-installed` / `self-updating` without changing anything. Exit code `10` when any update is available, `1` when a check is inconclusive, `0` when everything is current; `--json` is supported. Built for CI gates, cron checks, and shell-prompt badges. Kiro reports as self-updating because it updates itself in the background, and config `[channels]` overrides do not affect `check` (a tool's latest release is independent of how it was installed).

## 0.2.2 - 2026-06-12

### Fixed

- `--only` and `--except` are now global flags, matching `--json`, so filters work after subcommands such as `sync-ai-clis doctor --only gemini --json`.
- Config `[channels]` overrides are now validated and applied to installed-tool update planning. Unknown tool IDs or channel names fail fast with usage error exit 2. Missing-tool installs and `doctor` diagnostics still use the real detected state.
- Codex native installs now update with the official `codex update` command. Installer reruns remain the install and broken-binary recovery path.
- Kiro Windows 11 installs now use the confirmed official PowerShell installer, and native Kiro updates run `kiro-cli update --non-interactive`.
- Codex and Kiro now have confirmed `install_dir` values for PATH-refresh rechecks. Gemini is explicitly left without a fixed `install_dir` because npm and Homebrew prefixes vary by installation.

## 0.2.1 - 2026-06-11

### Fixed

- npm updates now run through the npm that owns the existing install (resolved from the install's real location and invoked by absolute path). A bare `npm install -g` resolves to whichever npm is active — under nvm that meant installing a second copy into a different prefix instead of updating. `--dry-run` shows the exact invocation.
- Homebrew formulas that vendor npm packages inside their Cellar (e.g. `gemini-cli`) are classified as brew installs again: Cellar/Caskroom paths now take precedence over the `node_modules` marker, so these tools update via `brew upgrade` instead of npm. npm globals installed through a brew-managed Node still classify as npm.

## 0.2.0 - 2026-06-11

### Added

- `sync-ai-clis doctor`: read-only diagnosis of unhealthy installs. Scans every PATH entry plus each tool's known install directory and reports duplicate installs (every copy with its install source and version, the copy PATH picks first, and a warning when an older copy shadows a newer one), broken installs (`--version` fails), and installed-but-not-on-PATH cases. Missing tools are informational. Exit 1 when issues are found, 0 when clean; supports `--json`.

### Changed

- `--json` is now a global flag, so it can follow subcommands: `sync-ai-clis doctor --json`.

## 0.1.3 - 2026-06-11

### Documentation

- README refresh in both languages, now shipped to the crates.io and npm landing pages (they render the README bundled at publish time): live channel badges, a terminal demo of `--dry-run`, tighter prose, and a simpler tagline.

### Fixed

- The WinGet publish workflow passes the release tag explicitly; it previously resolved to the default branch and could not find the release.

## 0.1.2 - 2026-06-11

### Changed

- `--dry-run` now renders the pending result version as `(dry-run)` instead of `(none)` — nothing was executed, so the version is pending, not gone (first real-user feedback).
- Updates that end on the same version are now marked `already current`, making idempotent update runs explicit.
- `--json` output is unchanged: it keeps raw values (`after` stays `null` under `--dry-run`).

## 0.1.1 - 2026-06-11

### Fixed

- Linux and macOS release archives are now `.tar.gz` instead of `.tar.xz`, so the npm wrapper and the shell installer work on minimal environments without `xz` installed (e.g. slim container images, bare CI runners).

## 0.1.0 - 2026-06-11

First release: one command to detect, install (with consent), update, and re-verify Claude Code, Codex, Gemini, Kiro, and Antigravity across macOS, Windows, and Linux.

### Added

- **Engine pipeline** (detect → plan → consent → run → verify → record) with continue-on-error across tools, before/after version capture, real `--version` re-verification, broken-install recovery (Codex reinstall), and PATH-refresh advice for fresh installs.
- **Declarative 5-tool registry** — install plans per OS and update plans per detected install source, with hardcoded official HTTPS installer URLs only; unsupported combinations skip with clear reasons (e.g. Kiro on Windows 10).
- **Install-source detection**: tools already installed via Homebrew, npm, winget, or Scoop are updated through that same channel (symlink-resolving path classification).
- **OS detection** including the Windows 11 gate (Kiro) and glibc-vs-musl classification (Linux).
- **CLI** per the SPEC contract: `--yes`, `--no-install`, `--only`/`--except`, `--dry-run` (prints the exact commands, runs nothing), `--json` (machine-pure stdout), `list`/`status`; config at `~/.config/sync-ai-clis/config.toml` with flags taking precedence; exit codes 0/1/2.
- **Trust model**: registry-hardcoded HTTPS URLs only, consent before installs, transparent dry-run, no automatic privilege escalation, no telemetry.
- **Verification**: 84 offline tests (OS × state × source command-selection matrix, fake-tool engine fixtures, binary smoke), a Docker Linux integration harness (6-distro real install/update matrix), and CI on ubuntu/macos/windows including real-channel integration runs.
- **Distribution**: GitHub Releases with shell/PowerShell installers, Homebrew tap (`hyeonbungi/homebrew-tap`), npm package, MSI for winget (`hyeonbungi.sync-ai-clis`), Scoop bucket (`hyeonbungi/scoop-bucket`), and crates.io — built and published by `dist` v0.32.0.

### Notes

- Kiro's Windows 11 install command is not yet confirmed upstream; sync-ai-clis reports a clear SKIP on Windows for now (tracked in SPEC §11).
- The project grew out of a personal macOS-only bash updater script.
