## What & why

<!-- One main change per PR. What does it do, and what problem does it solve? -->

## Validation

<!-- Paste the commands you ran and their results. -->

```
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Checklist

- [ ] This PR contains one main change
- [ ] `cargo fmt --check`, `clippy -D warnings`, and `cargo test` all pass
- [ ] Tests added/updated (registry changes need command-selection cases in `tests/command_selection.rs`)
- [ ] Real installs/updates were exercised only in containers or CI — never on a development machine
- [ ] New/changed tool entries use official HTTPS URLs only; unverified channels return `Unsupported` with a reason
- [ ] README tool table updated if the registry changed

<!-- Versioning is maintainer-handled (SemVer 0.x — see CONTRIBUTING.md "Versioning"); no need to bump versions in PRs. -->
