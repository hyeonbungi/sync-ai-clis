//! `check` (design doc 0012): read-only "is an update available?" per tool.
//! Captures the installed version, asks each tool's declared `latest_source`
//! for the latest available version (npm registry / official manifest), and
//! compares with the shared best-effort version key. Read-only — never
//! installs or updates. Exit codes: 10 when any update is available, 1 when a
//! verdict is inconclusive, 0 when everything checkable is current.

use std::path::PathBuf;

use crate::os::OsInfo;
use crate::runner::{Command, CommandRunner};
use crate::tools::{Extract, LatestSource, ToolSpec};
use crate::version::version_key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Installed and at the latest known version.
    Current,
    /// Installed; a newer version is available.
    UpdateAvailable,
    /// Installed, but latest could not be determined (probe/parse failed, or
    /// the two versions are not comparable).
    Unknown,
    /// Not installed anywhere on PATH — informational, never an issue.
    NotInstalled,
    /// Tool keeps itself current in the background (Kiro).
    SelfUpdating,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckResult {
    pub id: &'static str,
    pub display: &'static str,
    pub installed: bool,
    /// Installed version (first line of `--version`), when probeable.
    pub current: Option<String>,
    /// Latest available version, when determinable.
    pub latest: Option<String>,
    pub status: Status,
    /// Short explanation for unknown / self-updating cases.
    pub note: Option<String>,
}

/// Checks one tool read-only: resolves the installed version and the declared
/// latest source, then compares. `find_bin` locates the binary on PATH;
/// `probe` runs the read-only version queries (`--version`, `npm view`,
/// `curl <manifest>`).
pub fn check_tool(
    tool: &ToolSpec,
    os: &OsInfo,
    find_bin: &dyn Fn(&str) -> Option<PathBuf>,
    probe: &mut dyn CommandRunner,
) -> CheckResult {
    let mut result = CheckResult {
        id: tool.id,
        display: tool.display,
        installed: false,
        current: None,
        latest: None,
        status: Status::NotInstalled,
        note: None,
    };

    if find_bin(tool.bin).is_none() {
        return result;
    }
    result.installed = true;
    result.current = capture_first_line(probe, &Command::new(tool.bin, tool.version_args));

    match (tool.latest_source)(os) {
        LatestSource::SelfUpdating => {
            result.status = Status::SelfUpdating;
            result.note = Some("updates itself in the background".to_string());
        }
        LatestSource::Probe { command, extract } => {
            match latest_version(probe, &command, extract) {
                Some(latest) => {
                    result.status = compare(result.current.as_deref(), &latest, &mut result.note);
                    result.latest = Some(latest);
                }
                None => {
                    result.status = Status::Unknown;
                    result.note = Some("could not read the latest version".to_string());
                }
            }
        }
    }
    result
}

/// Runs a read-only probe and returns its trimmed first line, or None on
/// failure / empty output.
fn capture_first_line(probe: &mut dyn CommandRunner, cmd: &Command) -> Option<String> {
    match probe.capture(cmd) {
        Ok(cap) if cap.success => {
            let line = cap.stdout.lines().next().unwrap_or("").trim();
            (!line.is_empty()).then(|| line.to_string())
        }
        _ => None,
    }
}

/// Runs the latest-source probe and pulls the version out per `extract`.
fn latest_version(
    probe: &mut dyn CommandRunner,
    cmd: &Command,
    extract: Extract,
) -> Option<String> {
    let cap = probe.capture(cmd).ok()?;
    if !cap.success {
        return None;
    }
    match extract {
        Extract::Raw => {
            let line = cap.stdout.lines().next().unwrap_or("").trim();
            (!line.is_empty()).then(|| line.to_string())
        }
        Extract::JsonKey(key) => {
            let value: serde_json::Value = serde_json::from_str(&cap.stdout).ok()?;
            value.get(key)?.as_str().map(str::to_string)
        }
    }
}

/// Compares installed vs latest with the shared best-effort key, recording a
/// note when the two cannot be compared.
fn compare(current: Option<&str>, latest: &str, note: &mut Option<String>) -> Status {
    match (current.and_then(version_key), version_key(latest)) {
        (Some(c), Some(l)) if l > c => Status::UpdateAvailable,
        (Some(_), Some(_)) => Status::Current,
        _ => {
            *note = Some("versions are not comparable".to_string());
            Status::Unknown
        }
    }
}

/// Exit code (design doc 0012): 10 when any update is available, else 1 when
/// any verdict is inconclusive, else 0. NotInstalled/SelfUpdating are neutral.
pub fn exit_code(results: &[CheckResult]) -> i32 {
    if results.iter().any(|r| r.status == Status::UpdateAvailable) {
        10
    } else if results.iter().any(|r| r.status == Status::Unknown) {
        1
    } else {
        0
    }
}

/// Human table: one line per tool (display, version transition, status), then
/// a summary count. Plain text so it stays easy to assert on (like doctor).
pub fn render(results: &[CheckResult]) -> String {
    let width = results.iter().map(|r| r.display.len()).max().unwrap_or(0);
    let mut lines: Vec<String> = results
        .iter()
        .map(|r| format!("{:<w$}  {}", r.display, status_line(r), w = width))
        .collect();
    let updates = results
        .iter()
        .filter(|r| r.status == Status::UpdateAvailable)
        .count();
    lines.push(String::new());
    lines.push(match updates {
        0 => "All checked tools are up to date.".to_string(),
        1 => "1 update available.".to_string(),
        n => format!("{n} updates available."),
    });
    lines.join("\n")
}

fn status_line(r: &CheckResult) -> String {
    let current = r.current.as_deref().unwrap_or("?");
    match r.status {
        Status::UpdateAvailable => format!(
            "{current} -> {}   update available",
            r.latest.as_deref().unwrap_or("?")
        ),
        Status::Current => format!("{current}   up to date"),
        Status::Unknown => format!(
            "{current}   unknown ({})",
            r.note.as_deref().unwrap_or("could not determine latest")
        ),
        Status::NotInstalled => "not installed".to_string(),
        Status::SelfUpdating => format!("{current}   self-updating (auto)"),
    }
}

/// `--json` rows: [{id, display, installed, current, latest, status, note}]
/// (SPEC §6.3).
pub fn json_check(results: &[CheckResult]) -> String {
    let rows: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "display": r.display,
                "installed": r.installed,
                "current": r.current,
                "latest": r.latest,
                "status": status_str(r.status),
                "note": r.note,
            })
        })
        .collect();
    serde_json::to_string_pretty(&rows).expect("check rows serialize")
}

fn status_str(status: Status) -> &'static str {
    match status {
        Status::Current => "current",
        Status::UpdateAvailable => "update-available",
        Status::Unknown => "unknown",
        Status::NotInstalled => "not-installed",
        Status::SelfUpdating => "self-updating",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::Os;
    use crate::runner::MockRunner;
    use crate::tools::Support::{Supported, Unsupported};

    fn macos() -> OsInfo {
        OsInfo {
            os: Os::MacOs,
            arch: "aarch64".into(),
            windows_build: None,
            libc: None,
        }
    }

    /// Offline fixture with an npm-style `Raw` latest source.
    fn npm_tool() -> ToolSpec {
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
            latest_source: |_| LatestSource::Probe {
                command: Command::new("npm", &["view", "footool", "version"]),
                extract: Extract::Raw,
            },
        }
    }

    fn run(
        tool: &ToolSpec,
        find: &dyn Fn(&str) -> Option<PathBuf>,
        probe: &mut MockRunner,
    ) -> CheckResult {
        check_tool(tool, &macos(), find, probe)
    }

    #[test]
    fn update_available_when_latest_is_newer() {
        let mut probe = MockRunner::new();
        probe.script_capture("footool --version", true, "footool 1.0.0");
        probe.script_capture("npm view footool version", true, "1.1.0");
        let r = run(
            &npm_tool(),
            &|_| Some(PathBuf::from("/usr/local/bin/footool")),
            &mut probe,
        );
        assert_eq!(r.status, Status::UpdateAvailable);
        assert_eq!(r.current.as_deref(), Some("footool 1.0.0"));
        assert_eq!(r.latest.as_deref(), Some("1.1.0"));
        assert!(r.installed);
    }

    #[test]
    fn current_when_latest_equals_installed() {
        let mut probe = MockRunner::new();
        probe.script_capture("footool --version", true, "1.2.3");
        probe.script_capture("npm view footool version", true, "1.2.3");
        let r = run(
            &npm_tool(),
            &|_| Some(PathBuf::from("/x/footool")),
            &mut probe,
        );
        assert_eq!(r.status, Status::Current);
        assert_eq!(r.latest.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn unknown_when_latest_probe_fails() {
        let mut probe = MockRunner::new();
        probe.script_capture("footool --version", true, "1.0.0");
        probe.script_capture("npm view footool version", false, "");
        let r = run(
            &npm_tool(),
            &|_| Some(PathBuf::from("/x/footool")),
            &mut probe,
        );
        assert_eq!(r.status, Status::Unknown);
        assert_eq!(r.latest, None);
        assert!(r.note.is_some());
    }

    #[test]
    fn unknown_when_versions_not_comparable() {
        let mut probe = MockRunner::new();
        probe.script_capture("footool --version", true, "weird-build");
        probe.script_capture("npm view footool version", true, "also-weird");
        let r = run(
            &npm_tool(),
            &|_| Some(PathBuf::from("/x/footool")),
            &mut probe,
        );
        assert_eq!(r.status, Status::Unknown);
    }

    #[test]
    fn not_installed_when_binary_absent() {
        let mut probe = MockRunner::new();
        let r = run(&npm_tool(), &|_| None, &mut probe);
        assert_eq!(r.status, Status::NotInstalled);
        assert!(!r.installed);
        assert!(probe.calls.is_empty(), "no probing when not installed");
    }

    #[test]
    fn self_updating_tool_is_reported_not_compared() {
        let tool = ToolSpec {
            latest_source: |_| LatestSource::SelfUpdating,
            ..npm_tool()
        };
        let mut probe = MockRunner::new();
        probe.script_capture("footool --version", true, "2.0.0");
        let r = run(&tool, &|_| Some(PathBuf::from("/x/footool")), &mut probe);
        assert_eq!(r.status, Status::SelfUpdating);
        assert_eq!(r.current.as_deref(), Some("2.0.0"));
        assert_eq!(r.latest, None);
    }

    #[test]
    fn json_manifest_latest_is_parsed() {
        let tool = ToolSpec {
            latest_source: |_| LatestSource::Probe {
                command: Command::new(
                    "curl",
                    &["-fsSL", "https://example/manifests/darwin_arm64.json"],
                ),
                extract: Extract::JsonKey("version"),
            },
            ..npm_tool()
        };
        let mut probe = MockRunner::new();
        probe.script_capture("footool --version", true, "1.0.7");
        probe.script_capture(
            "curl -fsSL https://example/manifests/darwin_arm64.json",
            true,
            r#"{"version":"1.0.8","url":"x","sha512":"y"}"#,
        );
        let r = run(&tool, &|_| Some(PathBuf::from("/x/footool")), &mut probe);
        assert_eq!(r.status, Status::UpdateAvailable);
        assert_eq!(r.latest.as_deref(), Some("1.0.8"));
    }

    #[test]
    fn exit_code_prioritizes_update_then_unknown() {
        let mk = |status| CheckResult {
            id: "footool",
            display: "Foo Tool",
            installed: true,
            current: None,
            latest: None,
            status,
            note: None,
        };
        assert_eq!(
            exit_code(&[mk(Status::UpdateAvailable), mk(Status::Unknown)]),
            10
        );
        assert_eq!(exit_code(&[mk(Status::Unknown), mk(Status::Current)]), 1);
        assert_eq!(
            exit_code(&[
                mk(Status::Current),
                mk(Status::NotInstalled),
                mk(Status::SelfUpdating)
            ]),
            0
        );
        assert_eq!(exit_code(&[]), 0);
    }

    fn result(
        display: &'static str,
        status: Status,
        current: &str,
        latest: Option<&str>,
    ) -> CheckResult {
        CheckResult {
            id: "x",
            display,
            installed: true,
            current: Some(current.to_string()),
            latest: latest.map(str::to_string),
            status,
            note: None,
        }
    }

    #[test]
    fn render_shows_transition_and_summary() {
        let results = vec![
            result(
                "Claude Code",
                Status::UpdateAvailable,
                "2.1.170",
                Some("2.1.178"),
            ),
            result("Codex CLI", Status::Current, "0.140.0", Some("0.140.0")),
            result("Kiro CLI", Status::SelfUpdating, "2.0.0", None),
        ];
        let out = render(&results);
        assert!(out.contains("Claude Code"), "{out}");
        assert!(out.contains("2.1.170 -> 2.1.178"), "{out}");
        assert!(out.contains("update available"), "{out}");
        assert!(out.contains("up to date"), "{out}");
        assert!(out.contains("self-updating (auto)"), "{out}");
        assert!(out.contains("1 update available."), "{out}");
    }

    #[test]
    fn json_check_matches_the_documented_schema() {
        let results = vec![result(
            "Claude Code",
            Status::UpdateAvailable,
            "2.1.170",
            Some("2.1.178"),
        )];
        let json = json_check(&results);
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let row = &value.as_array().unwrap()[0];
        assert_eq!(row["display"], "Claude Code");
        assert_eq!(row["installed"], true);
        assert_eq!(row["current"], "2.1.170");
        assert_eq!(row["latest"], "2.1.178");
        assert_eq!(row["status"], "update-available");
        assert_eq!(row["note"], serde_json::Value::Null);
    }
}
