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

/// One result line for a tool: `display  before -> after  LABEL (note)`.
/// Under `dry_run` a pending after-version renders as `(dry-run)` (TD-006).
pub fn tool_line(report: &ToolReport, color: bool, dry_run: bool) -> String {
    let mut line = format!(
        "{}  {} -> {}  {}",
        report.display,
        version_or_none(&report.before),
        after_text(report, dry_run),
        outcome_label(report.outcome, color)
    );
    if let Some(note) = note_text(report) {
        line.push_str(&format!(" ({note})"));
    }
    line
}

/// Final aligned summary table (original-script style), one row per tool.
pub fn summary_table(reports: &[ToolReport], color: bool, dry_run: bool) -> String {
    let width = |f: &dyn Fn(&ToolReport) -> usize| reports.iter().map(f).max().unwrap_or(0);
    let display_width = width(&|r| r.display.len());
    let before_width = width(&|r| version_or_none(&r.before).len());
    let after_width = width(&|r| after_text(r, dry_run).len());

    reports
        .iter()
        .map(|r| {
            let mut row = format!(
                "  {:<display_width$}  {:<before_width$} -> {:<after_width$}  {}",
                r.display,
                version_or_none(&r.before),
                after_text(r, dry_run),
                outcome_label(r.outcome, color)
            );
            if let Some(note) = note_text(r) {
                row.push_str(&format!("  {note}"));
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

/// After-slot text: in dry-run nothing executed, so a missing after-version
/// means "pending", not "gone" — JSON keeps the raw null (SPEC §6.3).
fn after_text(report: &ToolReport, dry_run: bool) -> &str {
    if dry_run && report.after.is_none() {
        return "(dry-run)";
    }
    version_or_none(&report.after)
}

/// Trailing note: the engine's reason wins; otherwise an idempotent update
/// (same version before and after) is flagged as already current (SPEC §11).
fn note_text(report: &ToolReport) -> Option<&str> {
    if let Some(reason) = &report.reason {
        return Some(reason);
    }
    let unchanged = report.action == ActionKind::Update
        && report.outcome == Outcome::Ok
        && report.before.is_some()
        && report.before == report.after;
    unchanged.then_some("already current")
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
        let line = tool_line(&ok_report(), false, false);
        assert!(line.contains("Claude Code"), "line: {line}");
        assert!(line.contains("1.0.0 -> 1.1.0"), "line: {line}");
        assert!(line.contains("OK"), "line: {line}");

        let skip = tool_line(&skip_report(), false, false);
        assert!(skip.contains("(none)"), "missing versions render: {skip}");
        assert!(skip.contains("SKIP"), "line: {skip}");
        assert!(skip.contains("Windows 11"), "reason shown: {skip}");
    }

    #[test]
    fn summary_table_has_one_aligned_row_per_tool() {
        let table = summary_table(&[ok_report(), skip_report()], false, false);
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

#[cfg(test)]
mod rendering_polish_tests {
    use super::*;
    use crate::engine::{ActionKind, Outcome, ToolReport};

    fn report(before: Option<&str>, after: Option<&str>) -> ToolReport {
        ToolReport {
            id: "claude".into(),
            display: "Claude Code".into(),
            installed: true,
            before: before.map(String::from),
            after: after.map(String::from),
            action: ActionKind::Update,
            outcome: Outcome::Ok,
            reason: None,
            commands: vec!["claude update".into()],
        }
    }

    #[test]
    fn dry_run_renders_pending_result_not_none() {
        // TD-006 (first real-user feedback): "2.1.170 -> (none) OK" reads
        // like the version vanishes. Under dry-run the result is pending.
        let line = tool_line(&report(Some("2.1.170"), None), false, true);
        assert!(line.contains("2.1.170 -> (dry-run)"), "line: {line}");
        assert!(!line.contains("(none)"), "line: {line}");

        let table = summary_table(&[report(Some("2.1.170"), None)], false, true);
        assert!(table.contains("(dry-run)"), "table: {table}");
    }

    #[test]
    fn missing_versions_still_render_none_outside_dry_run() {
        let line = tool_line(&report(None, None), false, false);
        assert!(line.contains("(none) -> (none)"), "line: {line}");
    }

    #[test]
    fn unchanged_update_is_marked_already_current() {
        // SPEC §11 open question: idempotent updates should say so.
        let line = tool_line(&report(Some("2.1.170"), Some("2.1.170")), false, false);
        assert!(line.contains("already current"), "line: {line}");

        // A real upgrade must NOT carry the marker.
        let upgraded = tool_line(&report(Some("2.1.170"), Some("2.2.0")), false, false);
        assert!(!upgraded.contains("already current"), "line: {upgraded}");

        // The summary table marks it the same way.
        let table = summary_table(&[report(Some("2.1.170"), Some("2.1.170"))], false, false);
        assert!(table.contains("already current"), "table: {table}");
    }
}
