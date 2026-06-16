//! Best-effort version parsing shared by `doctor` and `check`: turn a tool's
//! `--version` output (or a registry's reported latest) into a comparable
//! numeric key. Deliberately lenient — only speaks up when a string parses.

/// First `digits(.digits)+` token, parsed numerically — `"2.1.170 (Claude
/// Code)"` → `[2, 1, 170]`. None when no such token exists.
pub fn version_key(version: &str) -> Option<Vec<u64>> {
    version
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .filter(|token| token.contains('.'))
        .find_map(|token| {
            let parts: Vec<u64> = token
                .split('.')
                .filter(|p| !p.is_empty())
                .map(str::parse)
                .collect::<Result<_, _>>()
                .ok()?;
            (parts.len() >= 2).then_some(parts)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_keys_parse_real_tool_outputs_best_effort() {
        assert_eq!(version_key("2.1.170 (Claude Code)"), Some(vec![2, 1, 170]));
        assert_eq!(version_key("codex-cli 0.139.0"), Some(vec![0, 139, 0]));
        assert_eq!(version_key("no digits here"), None);
    }
}
