# Contributing

Thank you for considering a contribution to `sync-ai-clis`.

This project detects, installs, and updates multiple AI coding CLIs across macOS, Windows, and Linux. The design — decisions, architecture, and the per-tool command matrix — lives in [SPEC.md](./SPEC.md); please skim it before proposing changes.

## Good Contributions

- Support for a new AI CLI in the tool registry (see below)
- Fixes for OS-, architecture-, or package-manager-specific edge cases (glibc/musl, Windows version gates, PATH refresh after install, …)
- Better detection of install sources (brew / npm / native / winget / scoop)
- Tests: command-selection cases (OS × install state), engine fixtures, Docker matrix legs
- Documentation fixes that reduce ambiguity

## Out Of Scope (v1)

- Running or orchestrating the AI CLIs themselves (this tool only installs/updates/verifies)
- Account, auth, or quota management for the tools
- GUI/TUI surfaces
- Telemetry of any kind
- Background daemons

## Adding A New AI CLI

Adding a tool is intentionally one module (SPEC.md §5.2):

1. Add `src/tools/<id>.rs` with a `ToolSpec` — bin name, version args, install plan per OS, update plan per install source, optional `on_broken` recovery hook, known `install_dir` for the PATH re-check.
2. Register it in `registry()` in `src/tools/mod.rs`.
3. Use **official HTTPS installer URLs only** — hardcoded constants, never user-supplied (SPEC.md §5.5). Unverified channels should return `Unsupported` with a clear reason instead of a guessed URL.
4. Add command-selection test cases in `tests/command_selection.rs` for each OS × install-state combination.
5. Update the tool table in `README.md` / `README.ko.md`.

## Before Opening A Pull Request

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
git diff --check
```

Real installs/updates must never run on your development machine — exercise them through `docker/run-matrix.sh` (disposable containers) or CI.

## Pull Request Expectations

- Keep each PR focused on one main change.
- Explain the problem being solved and the validation performed (commands + output).
- New dependencies need a recorded reason and considered alternatives.
- Don't claim broad platform support from a narrow test — say exactly what was verified where.

## Reporting Problems

Use GitHub issues for bugs, documentation gaps, or unclear behavior. For security-sensitive reports, follow [SECURITY.md](./SECURITY.md) instead.
