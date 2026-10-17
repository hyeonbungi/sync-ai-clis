//! End-to-end smoke of the built binary. Strictly read-only + dry-run:
//! never installs or updates anything (SPEC §8.5 — real execution only in
//! Docker/CI).

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sync-ai-clis"))
}

#[test]
fn removed_tools_are_rejected_before_any_network_or_install() {
    for id in ["gemini", "kiro"] {
        for args in [
            vec!["--dry-run", "--only", id],
            vec!["check", "--only", id],
            vec!["audit", "--only", id],
        ] {
            let out = bin().args(args).output().unwrap();
            assert_eq!(out.status.code(), Some(2));
            assert!(String::from_utf8_lossy(&out.stderr).contains("unknown tool id"));
        }
    }
    let out = bin().arg("list").output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!text.contains("Gemini"));
    assert!(!text.contains("Kiro"));
}

#[test]
fn json_stays_parseable_when_a_missing_install_is_declined() {
    use std::io::Write;
    use std::process::Stdio;
    let root = std::env::temp_dir().join(format!("sync-json-prompt-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let mut child = bin()
        .args(["--dry-run", "--json", "--only", "claude"])
        .env("PATH", "")
        .env("HOME", &root)
        .env("USERPROFILE", &root)
        .env("APPDATA", &root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"n\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let rows: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(rows[0]["id"], "claude");
    assert_eq!(rows[0]["result"], "skip");
    assert!(String::from_utf8_lossy(&out.stderr).contains("Install it?"));
    std::fs::remove_dir_all(root).unwrap();
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
    assert_eq!(rows.len(), 3, "all three tools reported");
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
    assert_eq!(rows.len(), 3, "all three tools diagnosed");
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
        .args(["doctor", "--only", "codex", "--json"])
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
    assert_eq!(rows.len(), 1, "only codex should be diagnosed: {stdout}");
    assert_eq!(rows[0]["id"], "codex");
}

#[test]
fn list_shows_all_known_tools_read_only() {
    let out = bin().arg("list").output().unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    for display in ["Claude Code", "Codex CLI", "Antigravity CLI"] {
        assert!(text.contains(display), "list missing {display}: {text}");
    }
}
