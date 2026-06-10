# Security Policy

`sync-ai-clis` executes remote official installer scripts (`curl | bash`, `irm | iex`) and package-manager commands on the user's machine. Its security posture is therefore part of the product design, not an afterthought. The authoritative trust model is [SPEC.md](./SPEC.md) §5.5.

## Trust Model

- **Hardcoded official URLs only.** Install/update URLs are HTTPS constants in the tool registry. Neither the config file nor CLI flags can inject arbitrary URLs or commands.
- **Consent before install.** Installing a missing tool requires an interactive `(y/N)` confirmation or an explicit `--yes`.
- **Transparent dry-run.** `--dry-run` prints exactly the commands that would be executed.
- **No automatic privilege escalation.** The tool never runs sudo or requests UAC elevation on its own. If elevation would be needed, it says so and lets the user act.
- **No telemetry.** Nothing is collected or transmitted beyond the tool commands themselves.

Violations of these invariants are security bugs — please report them.

## Supported Scope

The `main` branch is the supported version until the project publishes tagged releases (v0.1.0 planned in SPEC.md §10 Phase 2).

Security reports are appropriate for:

- Any way to make the tool execute a non-registry URL or command
- Privilege-escalation behavior
- Accidentally committed secrets or credentials
- Unsafe guidance in documentation (e.g., recommending unverified install sources)

## Reporting A Vulnerability

Use GitHub's private vulnerability reporting or security advisory flow when available for this repository.

If private reporting is not available, open a minimal public issue that says a security contact is needed. Do not include exploit details, secrets, tokens, private URLs, or personal data in a public issue.

## Handling Secrets

This tool needs no API keys or accounts of its own; the managed CLIs handle their own auth. Do not commit real secrets to this repository. Use placeholders in examples and store real values in environment variables or secret managers.

Before publishing changes, run:

```bash
if rg -n --glob '!SECURITY.md' --glob '!target' "(token|secret|password|api[_-]?key|credential)\s*[:=]|BEGIN .*PRIVATE KEY" .; then
  echo "Review matches before committing."
else
  echo "No obvious secrets found."
fi
```

Review matches before committing. Some documentation examples may intentionally contain placeholder words such as `secret`; real values must not appear.
