//! Rendering of engine results (SPEC §6.3): per-tool lines and a final
//! summary table in the original script's style, plus the --json schema
//! [{id, display, installed, before, after, action, result, reason}].
//!
//! Functions are pure string builders; callers print (main uses anstream so
//! colors strip on non-TTY). `color: false` renders plain text for tests
//! and --json runs.

use owo_colors::OwoColorize;

use crate::engine::{ActionKind, Outcome, ToolReport};

/// "OK" / "FAIL" / "SKIP", colored green/red/yellow when `color` is on.
pub fn outcome_label(outcome: Outcome, color: bool) -> String {
    let text = result_str(outcome).to_ascii_uppercase();
    if !color {
        return text;
    }
    match outcome {
        Outcome::Ok => text.green().to_string(),
        Outcome::Fail => text.red().to_string(),
        Outcome::Skip => text.yellow().to_string(),
    }
}

/// One result line for a tool: `display  before -> after  LABEL (reason)`.
pub fn tool_line(report: &ToolReport, color: bool) -> String {
    let mut line = format!(
        "{}  {} -> {}  {}",
        report.display,
        version_or_none(&report.before),
        version_or_none(&report.after),
        outcome_label(report.outcome, color)
    );
    if let Some(reason) = &report.reason {
        line.push_str(&format!(" ({reason})"));
    }
    line
}

/// Final aligned summary table (original-script style), one row per tool.
pub fn summary_table(reports: &[ToolReport], color: bool) -> String {
    let width = |f: &dyn Fn(&ToolReport) -> usize| reports.iter().map(f).max().unwrap_or(0);
    let display_width = width(&|r| r.display.len());
    let before_width = width(&|r| version_or_none(&r.before).len());
    let after_width = width(&|r| version_or_none(&r.after).len());

    reports
        .iter()
        .map(|r| {
            let mut row = format!(
                "  {:<display_width$}  {:<before_width$} -> {:<after_width$}  {}",
                r.display,
                version_or_none(&r.before),
                version_or_none(&r.after),
                outcome_label(r.outcome, color)
            );
            if let Some(reason) = &r.reason {
                row.push_str(&format!("  {reason}"));
            }
            row
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// SPEC §6.3 JSON: action install|update, result ok|fail|skip.
pub fn json_summary(reports: &[ToolReport]) -> String {
    let rows: Vec<serde_json::Value> = reports
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "display": r.display,
                "installed": r.installed,
                "before": r.before,
                "after": r.after,
                "action": action_str(r.action),
                "result": result_str(r.outcome),
                "reason": r.reason,
            })
        })
        .collect();
    serde_json::to_string_pretty(&rows).expect("report rows serialize")
}

fn version_or_none(version: &Option<String>) -> &str {
    version
        .as_deref()
        .filter(|v| !v.is_empty())
        .unwrap_or("(none)")
}

fn action_str(action: ActionKind) -> &'static str {
    match action {
        ActionKind::Install => "install",
        ActionKind::Update => "update",
    }
}

fn result_str(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Ok => "ok",
        Outcome::Fail => "fail",
        Outcome::Skip => "skip",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_report() -> ToolReport {
        ToolReport {
            id: "claude".into(),
            display: "Claude Code".into(),
            installed: true,
            before: Some("1.0.0".into()),
            after: Some("1.1.0".into()),
            action: ActionKind::Update,
            outcome: Outcome::Ok,
            reason: None,
            commands: vec!["claude update".into()],
        }
    }

    fn skip_report() -> ToolReport {
        ToolReport {
            id: "kiro".into(),
            display: "Kiro CLI".into(),
            installed: false,
            before: None,
            after: None,
            action: ActionKind::Install,
            outcome: Outcome::Skip,
            reason: Some("Kiro requires Windows 11 (SPEC §7.4)".into()),
            commands: vec![],
        }
    }

    #[test]
    fn outcome_labels_are_plain_without_color() {
        assert_eq!(outcome_label(Outcome::Ok, false), "OK");
        assert_eq!(outcome_label(Outcome::Fail, false), "FAIL");
        assert_eq!(outcome_label(Outcome::Skip, false), "SKIP");
        // Colored labels still contain the text.
        assert!(outcome_label(Outcome::Fail, true).contains("FAIL"));
    }

    #[test]
    fn tool_line_shows_versions_outcome_and_reason() {
        let line = tool_line(&ok_report(), false);
        assert!(line.contains("Claude Code"), "line: {line}");
        assert!(line.contains("1.0.0 -> 1.1.0"), "line: {line}");
        assert!(line.contains("OK"), "line: {line}");

        let skip = tool_line(&skip_report(), false);
        assert!(skip.contains("(none)"), "missing versions render: {skip}");
        assert!(skip.contains("SKIP"), "line: {skip}");
        assert!(skip.contains("Windows 11"), "reason shown: {skip}");
    }

    #[test]
    fn summary_table_has_one_aligned_row_per_tool() {
        let table = summary_table(&[ok_report(), skip_report()], false);
        let rows: Vec<&str> = table.lines().collect();
        assert_eq!(rows.len(), 2, "table: {table}");
        assert!(rows[0].contains("Claude Code") && rows[0].contains("OK"));
        assert!(rows[1].contains("Kiro CLI") && rows[1].contains("SKIP"));
        // Outcome column aligns: same byte offset for both labels.
        assert_eq!(rows[0].find("OK"), rows[1].find("SK"), "table: {table}");
    }

    #[test]
    fn json_summary_matches_spec_schema() {
        let json = json_summary(&[ok_report(), skip_report()]);
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let rows = value.as_array().unwrap();
        assert_eq!(rows.len(), 2);

        assert_eq!(rows[0]["id"], "claude");
        assert_eq!(rows[0]["display"], "Claude Code");
        assert_eq!(rows[0]["installed"], true);
        assert_eq!(rows[0]["before"], "1.0.0");
        assert_eq!(rows[0]["after"], "1.1.0");
        assert_eq!(rows[0]["action"], "update");
        assert_eq!(rows[0]["result"], "ok");
        assert_eq!(rows[0]["reason"], serde_json::Value::Null);

        assert_eq!(rows[1]["installed"], false);
        assert_eq!(rows[1]["before"], serde_json::Value::Null);
        assert_eq!(rows[1]["action"], "install");
        assert_eq!(rows[1]["result"], "skip");
        assert!(rows[1]["reason"].as_str().unwrap().contains("Windows 11"));
    }
}
