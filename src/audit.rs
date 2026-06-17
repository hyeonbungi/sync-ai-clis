//! `audit` (design doc 0013): read-only detection of changes in each tool's
//! remote install script. Fetches the current script, compares it against the
//! last accepted baseline, and reports unchanged / changed / unregistered /
//! unavailable / not-applicable. Read-only — only `accept_tool` (behind
//! `--accept`) writes a baseline. Exit codes: 10 when any script changed, 1
//! when a fetch was inconclusive, 0 otherwise.

use crate::baseline::BaselineStore;
use crate::os::OsInfo;
use crate::runner::{Command, CommandRunner};
use crate::tools::ToolSpec;

/// Per-tool audit verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Current script matches the accepted baseline.
    Unchanged,
    /// Current script differs from the baseline (diff attached).
    Changed,
    /// No baseline yet — `audit --accept` registers one (audit never writes).
    Unregistered,
    /// The script could not be fetched (network/HTTP failure).
    Unavailable,
    /// The tool has no remote install script (npm-managed, like gemini).
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditResult {
    pub id: &'static str,
    pub display: &'static str,
    pub status: Status,
    /// The install script URL that was checked, when one exists.
    pub url: Option<String>,
    /// Unified diff (baseline → current), present only when Changed.
    pub diff: Option<String>,
}

/// Audits one tool read-only: fetches the current install script and compares
/// it to the accepted baseline. Never writes — `accept_tool` is the only write
/// path. `fetch` runs the read-only `curl` GET (real, or MockRunner in tests).
pub fn audit_tool(
    tool: &ToolSpec,
    os: &OsInfo,
    fetch: &mut dyn CommandRunner,
    store: &BaselineStore,
) -> AuditResult {
    let (url, content) = match fetch_current(tool, os, fetch) {
        Ok(pair) => pair,
        Err(result) => return result,
    };
    let (status, diff) = match store.load(tool.id) {
        None => (Status::Unregistered, None),
        Some(baseline) if baseline == content => (Status::Unchanged, None),
        Some(baseline) => (Status::Changed, Some(unified_diff(&baseline, &content))),
    };
    AuditResult {
        id: tool.id,
        display: tool.display,
        status,
        url: Some(url),
        diff,
    }
}

/// Accepts the current script as the new baseline for one tool (the only write
/// path, behind `--accept`). Fetches, then records it; reports `Unchanged`
/// (now matching) on success.
pub fn accept_tool(
    tool: &ToolSpec,
    os: &OsInfo,
    fetch: &mut dyn CommandRunner,
    store: &BaselineStore,
) -> AuditResult {
    let (url, content) = match fetch_current(tool, os, fetch) {
        Ok(pair) => pair,
        Err(result) => return result,
    };
    let status = match store.save(tool.id, &content) {
        Ok(()) => Status::Unchanged,
        Err(_) => Status::Unavailable,
    };
    AuditResult {
        id: tool.id,
        display: tool.display,
        status,
        url: Some(url),
        diff: None,
    }
}

/// Fetches the current script. `Ok((url, content))` on success; `Err(result)`
/// carries the terminal NotApplicable/Unavailable verdict to return as-is.
fn fetch_current(
    tool: &ToolSpec,
    os: &OsInfo,
    fetch: &mut dyn CommandRunner,
) -> Result<(String, String), AuditResult> {
    let terminal = |status, url| AuditResult {
        id: tool.id,
        display: tool.display,
        status,
        url,
        diff: None,
    };
    let Some(url) = (tool.install_script)(os) else {
        return Err(terminal(Status::NotApplicable, None));
    };
    match fetch.capture(&Command::new("curl", &["-fsSL", url])) {
        Ok(cap) if cap.success => Ok((url.to_string(), cap.stdout)),
        _ => Err(terminal(Status::Unavailable, Some(url.to_string()))),
    }
}

/// Line-level unified diff (baseline → current) via the `similar` crate.
fn unified_diff(baseline: &str, current: &str) -> String {
    let diff = similar::TextDiff::from_lines(baseline, current);
    let mut output = diff.unified_diff();
    output.header("baseline", "current");
    output.to_string()
}

/// Exit code: 10 when any script changed (the signal CI/cron watches for),
/// else 1 when any fetch was inconclusive, else 0. Unregistered/NotApplicable
/// are neutral.
pub fn exit_code(results: &[AuditResult]) -> i32 {
    if results.iter().any(|r| r.status == Status::Changed) {
        10
    } else if results.iter().any(|r| r.status == Status::Unavailable) {
        1
    } else {
        0
    }
}

/// Human output: one line per tool (display + status), the unified diff
/// indented under any changed tool, then a summary. Plain text like
/// doctor/check so it stays easy to assert on and has no color dependency.
pub fn render(results: &[AuditResult]) -> String {
    let width = results.iter().map(|r| r.display.len()).max().unwrap_or(0);
    let mut lines: Vec<String> = Vec::new();
    for r in results {
        lines.push(format!("{:<width$}  {}", r.display, status_label(r.status)));
        if let Some(diff) = &r.diff {
            for line in diff.lines() {
                lines.push(format!("    {line}"));
            }
        }
    }
    lines.push(String::new());
    let changed = results
        .iter()
        .filter(|r| r.status == Status::Changed)
        .count();
    let unchanged = results
        .iter()
        .filter(|r| r.status == Status::Unchanged)
        .count();
    lines.push(match (changed, unchanged) {
        (0, 0) => "No baselines recorded yet.".to_string(),
        (0, _) => "All tracked scripts unchanged.".to_string(),
        (1, _) => "1 script changed — review the diff above.".to_string(),
        (n, _) => format!("{n} scripts changed — review the diffs above."),
    });
    if results.iter().any(|r| r.status == Status::Unregistered) {
        lines.push(
            "Some scripts are unregistered — run `audit --accept` to set the baseline.".to_string(),
        );
    }
    lines.join("\n")
}

fn status_label(status: Status) -> &'static str {
    match status {
        Status::Unchanged => "unchanged",
        Status::Changed => "changed",
        Status::Unregistered => "unregistered",
        Status::Unavailable => "unavailable (could not fetch)",
        Status::NotApplicable => "not applicable (no remote install script)",
    }
}

/// `--json` rows: [{id, display, status, url, diff}] (SPEC §6.3). `diff` is
/// non-null only for changed scripts.
pub fn json_audit(results: &[AuditResult]) -> String {
    let rows: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "display": r.display,
                "status": status_str(r.status),
                "url": r.url,
                "diff": r.diff,
            })
        })
        .collect();
    serde_json::to_string_pretty(&rows).expect("audit rows serialize")
}

fn status_str(status: Status) -> &'static str {
    match status {
        Status::Unchanged => "unchanged",
        Status::Changed => "changed",
        Status::Unregistered => "unregistered",
        Status::Unavailable => "unavailable",
        Status::NotApplicable => "not-applicable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::Os;
    use crate::runner::MockRunner;
    use crate::tools::LatestSource;
    use crate::tools::Support::{Supported, Unsupported};

    fn macos() -> OsInfo {
        OsInfo {
            os: Os::MacOs,
            arch: "aarch64".into(),
            windows_build: None,
            libc: None,
        }
    }

    /// Offline fixture with a remote install script.
    fn script_tool() -> ToolSpec {
        ToolSpec {
            id: "footool",
            display: "Foo Tool",
            bin: "footool",
            version_args: &["--version"],
            install_dir: |_| None,
            self_updates: false,
            install: |_| Supported(vec![]),
            update: |_, _| Unsupported("n/a"),
            on_broken: None,
            latest_source: |_| LatestSource::SelfUpdating,
            install_script: |_| Some("https://example/install.sh"),
        }
    }

    /// Fixture with no remote install script (npm-managed, like gemini).
    fn no_script_tool() -> ToolSpec {
        ToolSpec {
            install_script: |_| None,
            ..script_tool()
        }
    }

    fn temp_store(name: &str) -> BaselineStore {
        let dir =
            std::env::temp_dir().join(format!("sync-audit-core-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        BaselineStore::new(dir)
    }

    const FETCH: &str = "curl -fsSL https://example/install.sh";

    #[test]
    fn unchanged_when_content_matches_baseline() {
        let store = temp_store("unchanged");
        store.save("footool", "body-v1\n").unwrap();
        let mut fetch = MockRunner::new();
        fetch.script_capture(FETCH, true, "body-v1\n");
        let r = audit_tool(&script_tool(), &macos(), &mut fetch, &store);
        assert_eq!(r.status, Status::Unchanged);
        assert_eq!(r.diff, None);
    }

    #[test]
    fn changed_includes_diff() {
        let store = temp_store("changed");
        store.save("footool", "line-a\nline-b\n").unwrap();
        let mut fetch = MockRunner::new();
        fetch.script_capture(FETCH, true, "line-a\nline-c\n");
        let r = audit_tool(&script_tool(), &macos(), &mut fetch, &store);
        assert_eq!(r.status, Status::Changed);
        let diff = r.diff.expect("diff present on change");
        assert!(
            diff.contains("line-b") && diff.contains("line-c"),
            "diff: {diff}"
        );
    }

    #[test]
    fn unregistered_when_no_baseline_and_never_writes() {
        let store = temp_store("unreg");
        let mut fetch = MockRunner::new();
        fetch.script_capture(FETCH, true, "body\n");
        let r = audit_tool(&script_tool(), &macos(), &mut fetch, &store);
        assert_eq!(r.status, Status::Unregistered);
        assert_eq!(store.load("footool"), None, "audit must not write");
    }

    #[test]
    fn unavailable_when_fetch_fails() {
        let store = temp_store("unavail");
        let mut fetch = MockRunner::new();
        fetch.script_capture(FETCH, false, "");
        let r = audit_tool(&script_tool(), &macos(), &mut fetch, &store);
        assert_eq!(r.status, Status::Unavailable);
    }

    #[test]
    fn not_applicable_when_no_install_script() {
        let store = temp_store("na");
        let mut fetch = MockRunner::new();
        let r = audit_tool(&no_script_tool(), &macos(), &mut fetch, &store);
        assert_eq!(r.status, Status::NotApplicable);
        assert!(fetch.calls.is_empty(), "no fetch when not applicable");
    }

    #[test]
    fn accept_writes_baseline() {
        let store = temp_store("accept");
        let mut fetch = MockRunner::new();
        fetch.script_capture(FETCH, true, "accepted-body\n");
        let r = accept_tool(&script_tool(), &macos(), &mut fetch, &store);
        assert_eq!(r.status, Status::Unchanged);
        assert_eq!(store.load("footool").as_deref(), Some("accepted-body\n"));
    }

    #[test]
    fn exit_code_prioritizes_changed_then_unavailable() {
        let mk = |status| AuditResult {
            id: "x",
            display: "X",
            status,
            url: None,
            diff: None,
        };
        assert_eq!(
            exit_code(&[mk(Status::Changed), mk(Status::Unavailable)]),
            10
        );
        assert_eq!(
            exit_code(&[mk(Status::Unavailable), mk(Status::Unchanged)]),
            1
        );
        assert_eq!(
            exit_code(&[
                mk(Status::Unchanged),
                mk(Status::Unregistered),
                mk(Status::NotApplicable)
            ]),
            0
        );
        assert_eq!(exit_code(&[]), 0);
    }

    fn result(display: &'static str, status: Status, diff: Option<&str>) -> AuditResult {
        AuditResult {
            id: "x",
            display,
            status,
            url: Some("https://example/install.sh".to_string()),
            diff: diff.map(str::to_string),
        }
    }

    #[test]
    fn render_shows_changed_with_diff_and_summary() {
        let results = vec![
            result(
                "Claude Code",
                Status::Changed,
                Some("@@ -1 +1 @@\n-old\n+new"),
            ),
            result("Antigravity CLI", Status::Unchanged, None),
        ];
        let out = render(&results);
        assert!(out.contains("Claude Code"), "{out}");
        assert!(out.contains("changed"), "{out}");
        assert!(out.contains("+new"), "diff body shown: {out}");
        assert!(out.contains("1 script changed"), "{out}");
    }

    #[test]
    fn render_first_run_reports_no_baselines_and_hints_accept() {
        let out = render(&[result("Claude Code", Status::Unregistered, None)]);
        assert!(out.contains("unregistered"), "per-tool status: {out}");
        assert!(out.contains("No baselines recorded yet"), "summary: {out}");
        assert!(out.contains("--accept"), "hint: {out}");
    }

    #[test]
    fn render_all_unchanged_summary() {
        let out = render(&[result("Claude Code", Status::Unchanged, None)]);
        assert!(out.contains("All tracked scripts unchanged"), "{out}");
    }

    #[test]
    fn json_audit_matches_schema() {
        let results = vec![
            result("Claude Code", Status::Changed, Some("DIFF")),
            AuditResult {
                id: "gemini",
                display: "Gemini CLI",
                status: Status::NotApplicable,
                url: None,
                diff: None,
            },
        ];
        let value: serde_json::Value = serde_json::from_str(&json_audit(&results)).unwrap();
        let arr = value.as_array().unwrap();
        assert_eq!(arr[0]["display"], "Claude Code");
        assert_eq!(arr[0]["status"], "changed");
        assert_eq!(arr[0]["url"], "https://example/install.sh");
        assert_eq!(arr[0]["diff"], "DIFF");
        assert_eq!(arr[1]["status"], "not-applicable");
        assert_eq!(arr[1]["url"], serde_json::Value::Null);
        assert_eq!(arr[1]["diff"], serde_json::Value::Null);
    }
}
