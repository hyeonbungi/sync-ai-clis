//! `doctor` (SPEC §6.1/§6.3, v0.2.0): read-only install diagnosis.
//! Finds every copy of each tool (all PATH entries + the known install
//! dir), classifies each copy's install source, probes each copy's
//! `--version`, and reports duplicates (PATH winner first), broken
//! installs, and installed-but-not-on-PATH cases. Missing tools are
//! informational — installing is `sync`'s job, not doctor's.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::os::OsInfo;
use crate::runner::{Command, CommandRunner};
use crate::source::{self, InstallSource};
use crate::tools::ToolSpec;
use crate::version::version_key;

/// One discovered copy of a tool's binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// Path as found (PATH entry or install_dir) — what the user sees.
    pub path: PathBuf,
    /// Install source classified from the symlink-resolved target.
    pub source: InstallSource,
    /// First line of `--version` output; None when the probe failed.
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Ok,
    /// Not installed anywhere — informational, `sync` handles installs.
    Missing,
    /// Distinct copies in more than one place; PATH order decides the winner.
    Duplicates,
    /// The copy PATH picks fails its `--version` probe.
    Broken,
    /// Present at the tool's known install dir, but that dir is not on PATH.
    NotOnPath,
}

#[derive(Debug)]
pub struct Diagnosis {
    pub id: &'static str,
    pub display: &'static str,
    pub health: Health,
    /// PATH winner first; deduplicated by symlink-resolved target.
    pub locations: Vec<Location>,
    pub advice: Option<String>,
}

/// Diagnoses one tool. Read-only: scans PATH and the known install dir,
/// then probes each distinct copy's `--version` through `probe`.
pub fn diagnose(
    tool: &ToolSpec,
    os: &OsInfo,
    path_var: &OsStr,
    probe: &mut dyn CommandRunner,
) -> Diagnosis {
    let hits = dedup_by_target(source::find_all_in_path_env(tool.bin, path_var));

    if hits.is_empty() {
        return diagnose_off_path(tool, os, probe);
    }

    let locations: Vec<Location> = hits
        .into_iter()
        .map(|path| probe_location(tool, path, probe))
        .collect();

    let (health, advice) = if locations[0].version.is_none() {
        (
            Health::Broken,
            Some(format!(
                "`{} --version` fails at {} — run `sync-ai-clis` to repair (reinstall) it",
                tool.bin,
                locations[0].path.display()
            )),
        )
    } else if locations.len() > 1 {
        (Health::Duplicates, Some(duplicates_advice(&locations)))
    } else {
        (Health::Ok, None)
    };

    Diagnosis {
        id: tool.id,
        display: tool.display,
        health,
        locations,
        advice,
    }
}

/// Nothing on PATH: the binary may still sit in the tool's known install
/// dir (same recheck the engine does after a fresh install, SPEC §5.5).
fn diagnose_off_path(tool: &ToolSpec, os: &OsInfo, probe: &mut dyn CommandRunner) -> Diagnosis {
    let off_path_bin = (tool.install_dir)(os)
        .map(|dir| dir.join(tool.bin))
        .filter(|bin| bin.is_file());

    match off_path_bin {
        Some(bin) => {
            let dir = bin.parent().map(Path::to_path_buf).unwrap_or_default();
            let location = probe_location(tool, bin, probe);
            Diagnosis {
                id: tool.id,
                display: tool.display,
                health: Health::NotOnPath,
                locations: vec![location],
                advice: Some(format!(
                    "installed at {} but that directory is not on PATH — add it to PATH or restart your shell",
                    dir.display()
                )),
            }
        }
        None => Diagnosis {
            id: tool.id,
            display: tool.display,
            health: Health::Missing,
            locations: vec![],
            advice: None,
        },
    }
}

fn probe_location(tool: &ToolSpec, path: PathBuf, probe: &mut dyn CommandRunner) -> Location {
    let source = source::detect_from_path(&path);
    let program = path.to_string_lossy();
    let version = match probe.capture(&Command::new(&program, tool.version_args)) {
        Ok(cap) if cap.success => Some(cap.stdout.lines().next().unwrap_or("").to_string()),
        _ => None,
    };
    Location {
        path,
        source,
        version,
    }
}

/// Two PATH entries pointing at the same resolved target are one install,
/// not a duplicate. Keeps first-seen (PATH winner) order.
fn dedup_by_target(hits: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut out = Vec::new();
    for hit in hits {
        let target = std::fs::canonicalize(&hit).unwrap_or_else(|_| hit.clone());
        if !seen.contains(&target) {
            seen.push(target);
            out.push(hit);
        }
    }
    out
}

fn duplicates_advice(locations: &[Location]) -> String {
    let winner = &locations[0];
    let describe = |l: &Location| {
        format!(
            "{} ({}, {})",
            l.path.display(),
            source_str(l.source),
            l.version.as_deref().unwrap_or("version unknown")
        )
    };
    let others = locations[1..]
        .iter()
        .map(describe)
        .collect::<Vec<_>>()
        .join("; ");

    // Best-effort version ordering: only speak up when both sides parse.
    let winner_key = winner.version.as_deref().and_then(version_key);
    let newest_other = locations[1..]
        .iter()
        .filter_map(|l| l.version.as_deref().and_then(version_key))
        .max();
    let shadow_note = match (winner_key, newest_other) {
        (Some(w), Some(n)) if w < n => "an older copy shadows a newer one — ",
        _ => "",
    };

    format!(
        "{shadow_note}PATH picks {}; also found: {others}. Remove the extra copies or reorder PATH",
        describe(winner)
    )
}

/// True when any diagnosis needs attention (missing tools do not).
pub fn has_issues(diagnoses: &[Diagnosis]) -> bool {
    diagnoses
        .iter()
        .any(|d| !matches!(d.health, Health::Ok | Health::Missing))
}

fn health_str(health: Health) -> &'static str {
    match health {
        Health::Ok => "ok",
        Health::Missing => "missing",
        Health::Duplicates => "duplicates",
        Health::Broken => "broken",
        Health::NotOnPath => "not-on-path",
    }
}

fn source_str(source: InstallSource) -> &'static str {
    match source {
        InstallSource::Native => "native",
        InstallSource::Brew => "brew",
        InstallSource::Npm => "npm",
        InstallSource::Winget => "winget",
        InstallSource::Scoop => "scoop",
    }
}

/// Human block for one tool: status line, then one line per location,
/// then advice. Callers print under their own `==> {display}` header.
pub fn render(diagnosis: &Diagnosis) -> String {
    let mut lines = vec![format!(
        "{}  {}",
        diagnosis.display,
        match diagnosis.health {
            Health::Ok => "ok".to_string(),
            Health::Missing => "not installed (run sync-ai-clis to install)".to_string(),
            Health::Duplicates => format!("duplicate installs ({})", diagnosis.locations.len()),
            Health::Broken => "broken (--version fails)".to_string(),
            Health::NotOnPath => "installed but not on PATH".to_string(),
        }
    )];
    for (index, location) in diagnosis.locations.iter().enumerate() {
        let marker = if index == 0 && diagnosis.locations.len() > 1 {
            "-> "
        } else {
            "   "
        };
        lines.push(format!(
            "  {marker}{} ({}, {})",
            location.path.display(),
            source_str(location.source),
            location.version.as_deref().unwrap_or("version unknown")
        ));
    }
    if let Some(advice) = &diagnosis.advice {
        lines.push(format!("  advice: {advice}"));
    }
    lines.join("\n")
}

/// `--json` rows: [{id, display, status, locations: [{path, source,
/// version}], advice}] (SPEC §6.3).
pub fn json_doctor(diagnoses: &[Diagnosis]) -> String {
    let rows: Vec<serde_json::Value> = diagnoses
        .iter()
        .map(|d| {
            serde_json::json!({
                "id": d.id,
                "display": d.display,
                "status": health_str(d.health),
                "locations": d.locations.iter().map(|l| {
                    serde_json::json!({
                        "path": l.path.to_string_lossy(),
                        "source": source_str(l.source),
                        "version": l.version,
                    })
                }).collect::<Vec<_>>(),
                "advice": d.advice,
            })
        })
        .collect();
    serde_json::to_string_pretty(&rows).expect("doctor rows serialize")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::os::{Os, OsInfo};
    use crate::runner::{Command, MockRunner};
    use crate::source::InstallSource;
    use crate::tools::Support::{Supported, Unsupported};
    use crate::tools::ToolSpec;
    use std::ffi::OsString;
    use std::path::PathBuf;

    fn macos() -> OsInfo {
        OsInfo {
            os: Os::MacOs,
            arch: "aarch64".into(),
            windows_build: None,
            libc: None,
        }
    }

    /// Offline fixture tool. `install_dir` points at a per-process temp dir
    /// (fn pointers cannot capture) used only by the not-on-PATH test.
    fn footool() -> ToolSpec {
        ToolSpec {
            id: "footool",
            display: "Foo Tool",
            bin: "footool",
            version_args: &["--version"],
            install_dir: |_| {
                Some(
                    std::env::temp_dir()
                        .join(format!("sync-doctor-installdir-{}", std::process::id())),
                )
            },
            self_updates: false,
            install: |_| Supported(vec![Command::sh("foo-install.sh")]),
            update: |_, source| match source {
                InstallSource::Native => Supported(vec![Command::new("footool", &["update"])]),
                _ => Unsupported("footool is native-only"),
            },
            on_broken: None,
            latest_source: |_| crate::tools::LatestSource::SelfUpdating,
        }
    }

    /// Tests run in parallel and `footool()`'s install_dir is per-process:
    /// tests that must NOT see it use this no-install-dir variant.
    fn footool_without_install_dir() -> ToolSpec {
        ToolSpec {
            install_dir: |_| None,
            ..footool()
        }
    }

    /// Temp dir tree unique to this test, with a fake `footool` in each
    /// requested subdir. Returns (root, full bin paths in subdir order).
    fn fake_bins(test: &str, subdirs: &[&str]) -> (PathBuf, Vec<PathBuf>) {
        let root = std::env::temp_dir().join(format!("sync-doctor-{test}-{}", std::process::id()));
        let mut bins = Vec::new();
        for sub in subdirs {
            let dir = root.join(sub);
            std::fs::create_dir_all(&dir).unwrap();
            let bin = dir.join("footool");
            std::fs::write(&bin, "#!/bin/sh\n").unwrap();
            bins.push(bin);
        }
        (root, bins)
    }

    fn path_var(dirs: &[PathBuf]) -> OsString {
        std::env::join_paths(dirs.iter().cloned()).unwrap()
    }

    fn script_version(probe: &mut MockRunner, bin: &Path, success: bool, stdout: &str) {
        let rendered = format!("{} --version", bin.display());
        probe.script_capture(&rendered, success, stdout);
    }

    #[test]
    fn healthy_single_install_is_ok() {
        let (root, bins) = fake_bins("ok", &["bin"]);
        let mut probe = MockRunner::new();
        script_version(&mut probe, &bins[0], true, "footool 1.2.3");

        let d = diagnose(
            &footool(),
            &macos(),
            &path_var(&[root.join("bin")]),
            &mut probe,
        );
        assert_eq!(d.health, Health::Ok, "advice: {:?}", d.advice);
        assert_eq!(d.locations.len(), 1);
        assert_eq!(d.locations[0].source, InstallSource::Native);
        assert_eq!(d.locations[0].version.as_deref(), Some("footool 1.2.3"));
        assert!(d.advice.is_none());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn duplicate_installs_report_the_path_winner_first() {
        // Second copy lives under a node_modules store -> classifies as npm.
        let (root, bins) = fake_bins("dup", &["bin", "lib/node_modules/footool/bin"]);
        let mut probe = MockRunner::new();
        script_version(&mut probe, &bins[0], true, "footool 2.0.0");
        script_version(&mut probe, &bins[1], true, "footool 3.0.0");

        let d = diagnose(
            &footool(),
            &macos(),
            &path_var(&[root.join("bin"), root.join("lib/node_modules/footool/bin")]),
            &mut probe,
        );
        assert_eq!(d.health, Health::Duplicates);
        assert_eq!(d.locations.len(), 2);
        assert_eq!(d.locations[0].path, bins[0], "PATH winner first");
        assert_eq!(d.locations[0].source, InstallSource::Native);
        assert_eq!(d.locations[1].source, InstallSource::Npm);
        let advice = d.advice.expect("duplicates carry advice");
        assert!(advice.contains("PATH picks"), "advice: {advice}");
        // Winner 2.0.0 is older than the shadowed 3.0.0 — call that out.
        assert!(advice.contains("newer"), "advice: {advice}");

        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn two_path_entries_to_the_same_target_are_not_duplicates() {
        let (root, bins) = fake_bins("alias", &["bin"]);
        let alias_dir = root.join("alias");
        std::fs::create_dir_all(&alias_dir).unwrap();
        std::os::unix::fs::symlink(&bins[0], alias_dir.join("footool")).unwrap();
        let mut probe = MockRunner::new();
        script_version(&mut probe, &bins[0], true, "footool 1.0.0");

        let d = diagnose(
            &footool(),
            &macos(),
            &path_var(&[root.join("bin"), alias_dir]),
            &mut probe,
        );
        assert_eq!(d.health, Health::Ok, "same canonical target, no dup");
        assert_eq!(d.locations.len(), 1);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn broken_winner_is_reported_with_repair_advice() {
        let (root, bins) = fake_bins("broken", &["bin"]);
        let mut probe = MockRunner::new();
        script_version(&mut probe, &bins[0], false, "");

        let d = diagnose(
            &footool(),
            &macos(),
            &path_var(&[root.join("bin")]),
            &mut probe,
        );
        assert_eq!(d.health, Health::Broken);
        assert_eq!(d.locations[0].version, None);
        let advice = d.advice.expect("broken carries advice");
        assert!(advice.contains("sync-ai-clis"), "advice: {advice}");

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn installed_but_not_on_path_is_flagged() {
        let tool = footool();
        let install_dir = (tool.install_dir)(&macos()).unwrap();
        std::fs::create_dir_all(&install_dir).unwrap();
        let bin = install_dir.join("footool");
        std::fs::write(&bin, "#!/bin/sh\n").unwrap();
        let mut probe = MockRunner::new();
        script_version(&mut probe, &bin, true, "footool 1.0.0");

        let empty = std::env::temp_dir().join(format!("sync-doctor-empty-{}", std::process::id()));
        std::fs::create_dir_all(&empty).unwrap();
        let d = diagnose(
            &tool,
            &macos(),
            &path_var(std::slice::from_ref(&empty)),
            &mut probe,
        );
        assert_eq!(d.health, Health::NotOnPath);
        assert_eq!(d.locations.len(), 1);
        let advice = d.advice.expect("not-on-PATH carries advice");
        assert!(advice.contains("PATH"), "advice: {advice}");

        std::fs::remove_dir_all(&install_dir).ok();
        std::fs::remove_dir(&empty).ok();
    }

    #[test]
    fn missing_everywhere_is_informational() {
        let empty = std::env::temp_dir().join(format!("sync-doctor-none-{}", std::process::id()));
        std::fs::create_dir_all(&empty).unwrap();
        let mut probe = MockRunner::new();

        let d = diagnose(
            &footool_without_install_dir(),
            &macos(),
            &path_var(std::slice::from_ref(&empty)),
            &mut probe,
        );
        assert_eq!(d.health, Health::Missing);
        assert!(d.locations.is_empty() && d.advice.is_none());
        assert!(
            !has_issues(std::slice::from_ref(&d)),
            "missing is not an issue"
        );

        std::fs::remove_dir(&empty).ok();
    }

    #[test]
    fn issues_are_duplicates_broken_and_not_on_path() {
        let mk = |health| Diagnosis {
            id: "footool",
            display: "Foo Tool",
            health,
            locations: vec![],
            advice: None,
        };
        assert!(!has_issues(&[mk(Health::Ok), mk(Health::Missing)]));
        assert!(has_issues(&[mk(Health::Duplicates)]));
        assert!(has_issues(&[mk(Health::Broken)]));
        assert!(has_issues(&[mk(Health::NotOnPath)]));
    }

    #[test]
    fn json_doctor_matches_the_documented_schema() {
        let d = Diagnosis {
            id: "footool",
            display: "Foo Tool",
            health: Health::Duplicates,
            locations: vec![Location {
                path: PathBuf::from("/x/bin/footool"),
                source: InstallSource::Brew,
                version: Some("1.0.0".into()),
            }],
            advice: Some("advice text".into()),
        };
        let json = json_doctor(std::slice::from_ref(&d));
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let row = &value.as_array().unwrap()[0];
        assert_eq!(row["id"], "footool");
        assert_eq!(row["display"], "Foo Tool");
        assert_eq!(row["status"], "duplicates");
        assert_eq!(row["locations"][0]["path"], "/x/bin/footool");
        assert_eq!(row["locations"][0]["source"], "brew");
        assert_eq!(row["locations"][0]["version"], "1.0.0");
        assert_eq!(row["advice"], "advice text");
    }

    #[test]
    fn human_rendering_shows_status_and_locations() {
        let (root, bins) = fake_bins("render", &["bin", "lib/node_modules/footool/bin"]);
        let mut probe = MockRunner::new();
        script_version(&mut probe, &bins[0], true, "footool 2.0.0");
        script_version(&mut probe, &bins[1], true, "footool 3.0.0");

        let d = diagnose(
            &footool(),
            &macos(),
            &path_var(&[root.join("bin"), root.join("lib/node_modules/footool/bin")]),
            &mut probe,
        );
        let block = render(&d);
        assert!(block.contains("Foo Tool"), "block: {block}");
        assert!(block.contains("duplicate"), "block: {block}");
        assert!(
            block.contains(&bins[0].display().to_string()),
            "block: {block}"
        );
        assert!(block.contains("npm"), "block: {block}");

        std::fs::remove_dir_all(&root).ok();
    }
}
