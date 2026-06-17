//! End-to-end smoke of the built binary. Strictly read-only + dry-run:
//! never installs or updates anything (SPEC §8.5 — real execution only in
//! Docker/CI).

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sync-ai-clis"))
}

#[test]
fn help_describes_the_real_cli() {
    let out = bin().arg("--help").output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    for expected in [
        "--yes",
        "--no-install",
        "--only",
        "--dry-run",
        "--json",
        "list",
        "check",
        "audit",
    ] {
        assert!(text.contains(expected), "help missing {expected}: {text}");
    }
}

#[test]
fn version_prints_cargo_version() {
    let out = bin().arg("--version").output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains(env!("CARGO_PKG_VERSION")), "got: {text}");
}

#[test]
fn conflicting_flags_exit_with_usage_error_2() {
    let out = bin().args(["--yes", "--no-install"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn unknown_tool_id_exits_with_usage_error_2() {
    let out = bin().args(["--only", "no-such-tool"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("no-such-tool"), "stderr: {err}");
}

#[test]
fn dry_run_json_emits_spec_schema_without_executing() {
    // Read-only: --dry-run never mutates; --no-install never prompts.
    let out = bin()
        .args(["--dry-run", "--no-install", "--json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("bad json ({e}): {stdout}"));
    let rows = value.as_array().unwrap();
    assert_eq!(rows.len(), 5, "all five tools reported");
    for row in rows {
        for key in [
            "id",
            "display",
            "installed",
            "before",
            "after",
            "action",
            "result",
            "reason",
        ] {
            assert!(row.get(key).is_some(), "row missing {key}: {row}");
        }
    }
}

#[test]
fn doctor_json_emits_diagnosis_schema_read_only() {
    // Read-only: doctor only probes --version, never installs or updates.
    let out = bin().args(["doctor", "--json"]).output().unwrap();
    assert!(
        matches!(out.status.code(), Some(0) | Some(1)),
        "doctor exits 0 (clean) or 1 (issues found), got {:?}",
        out.status.code()
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("bad json ({e}): {stdout}"));
    let rows = value.as_array().unwrap();
    assert_eq!(rows.len(), 5, "all five tools diagnosed");
    for row in rows {
        for key in ["id", "display", "status", "locations", "advice"] {
            assert!(row.get(key).is_some(), "row missing {key}: {row}");
        }
        let status = row["status"].as_str().unwrap();
        assert!(
            ["ok", "missing", "duplicates", "broken", "not-on-path"].contains(&status),
            "unexpected status: {status}"
        );
    }
}

#[test]
fn doctor_global_only_json_filters_to_one_tool_read_only() {
    // Read-only: doctor only probes --version, never installs or updates.
    let out = bin()
        .args(["doctor", "--only", "gemini", "--json"])
        .output()
        .unwrap();
    assert!(
        matches!(out.status.code(), Some(0) | Some(1)),
        "doctor exits 0 (clean) or 1 (issues found), got {:?}; stderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("bad json ({e}): {stdout}"));
    let rows = value.as_array().unwrap();
    assert_eq!(rows.len(), 1, "only gemini should be diagnosed: {stdout}");
    assert_eq!(rows[0]["id"], "gemini");
}

#[test]
fn list_shows_all_known_tools_read_only() {
    let out = bin().arg("list").output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    for display in [
        "Claude Code",
        "Codex CLI",
        "Gemini CLI",
        "Kiro CLI",
        "Antigravity CLI",
    ] {
        assert!(text.contains(display), "list missing {display}: {text}");
    }
}

#[test]
fn check_only_kiro_json_is_read_only_and_network_free() {
    // Kiro is SelfUpdating, so `check` never runs a latest-version probe for
    // it — this exercises check_cmd end-to-end (routing, --version probe, JSON,
    // exit code) without any network call. self-updating/not-installed are
    // both neutral, so the exit code is 0 either way (installed or not).
    let out = bin()
        .args(["check", "--only", "kiro", "--json"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("bad json ({e}): {stdout}"));
    let rows = value.as_array().unwrap();
    assert_eq!(rows.len(), 1, "only kiro should be checked: {stdout}");
    assert_eq!(rows[0]["id"], "kiro");
    for key in [
        "id",
        "display",
        "installed",
        "current",
        "latest",
        "status",
        "note",
    ] {
        assert!(rows[0].get(key).is_some(), "row missing {key}: {}", rows[0]);
    }
    let status = rows[0]["status"].as_str().unwrap();
    assert!(
        ["self-updating", "not-installed"].contains(&status),
        "unexpected kiro status: {status}"
    );
}

#[test]
fn audit_only_gemini_json_is_read_only_and_network_free() {
    // gemini is npm-managed with no remote install script, so `audit` reports
    // not-applicable and never fetches — this exercises audit_cmd end-to-end
    // (routing, JSON, exit code) without any network call. not-applicable is
    // neutral, so the exit code is 0.
    let out = bin()
        .args(["audit", "--only", "gemini", "--json"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "expected exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).unwrap_or_else(|e| panic!("bad json ({e}): {stdout}"));
    let rows = value.as_array().unwrap();
    assert_eq!(rows.len(), 1, "only gemini should be audited: {stdout}");
    assert_eq!(rows[0]["id"], "gemini");
    for key in ["id", "display", "status", "url", "diff"] {
        assert!(rows[0].get(key).is_some(), "row missing {key}: {}", rows[0]);
    }
    assert_eq!(rows[0]["status"], "not-applicable");
}
