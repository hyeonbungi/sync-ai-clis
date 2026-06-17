//! The per-tool pipeline: detect → plan → consent → run → verify → record
//! (SPEC §5.3). Behavior inherited from the original script: continue on
//! error across tools, before/after versions, verification means "--version
//! actually runs" (not just `command -v`), and on_broken recovery.
//!
//! All effects are injected — PATH lookup, read-only probes, mutating
//! execution, and the consent prompt — so the whole pipeline is testable
//! offline (SPEC §8.1/§8.4) and `--dry-run` falls out of the runner choice.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::os::{Os, OsInfo};
use crate::runner::{Command, CommandRunner};
use crate::source;
use crate::tools::{Support, ToolSpec};

/// How to treat tools that are not installed (SPEC §6: default prompt,
/// `--yes` → Always, `--no-install` → Never; config can pin it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallPolicy {
    Prompt,
    Always,
    Never,
}

/// What the engine decided to do for a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Install,
    Update,
}

/// Per-tool result category (SPEC §6.3): OK / FAIL / SKIP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Ok,
    Fail,
    Skip,
}

/// One row of the final report (SPEC §6.3 --json schema).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolReport {
    pub id: String,
    pub display: String,
    /// Whether the binary was on PATH before we did anything.
    pub installed: bool,
    pub before: Option<String>,
    pub after: Option<String>,
    pub action: ActionKind,
    pub outcome: Outcome,
    pub reason: Option<String>,
    /// Rendered commands that ran (or would run, under --dry-run).
    pub commands: Vec<String>,
}

/// Exit-code aggregation: true when no tool failed (SKIPs do not fail the
/// run — SPEC §6.3).
pub fn all_ok(reports: &[ToolReport]) -> bool {
    reports.iter().all(|r| r.outcome != Outcome::Fail)
}

/// npm updates must run through the npm that owns the existing install —
/// any other npm (nvm setups) installs into its own prefix and creates a
/// duplicate instead of an update (SPEC §11, found live 2026-06-11).
/// Rewriting happens before commands are rendered, so --dry-run shows the
/// real invocation (SPEC §5.5). Falls back to plain `npm` when the owner
/// cannot be resolved.
fn pin_npm_owner(
    cmds: Vec<Command>,
    installed_source: source::InstallSource,
    bin_path: &std::path::Path,
) -> Vec<Command> {
    if installed_source != source::InstallSource::Npm {
        return cmds;
    }
    let Some(owner) = source::owning_npm(bin_path) else {
        return cmds;
    };
    let owner = owner.to_string_lossy().into_owned();
    cmds.into_iter()
        .map(|mut cmd| {
            if cmd.program == "npm" {
                cmd.program = owner.clone();
            }
            cmd
        })
        .collect()
}

/// The engine with every effect injected. `probe` performs read-only
/// captures (`--version`) and must stay a real runner even under --dry-run;
/// `exec` performs mutations and is the dry-run switch point.
pub struct Engine<'a> {
    pub os: &'a OsInfo,
    pub find_bin: &'a dyn Fn(&str) -> Option<PathBuf>,
    pub probe: &'a mut dyn CommandRunner,
    pub exec: &'a mut dyn CommandRunner,
    pub consent: &'a mut dyn FnMut(&str) -> bool,
    pub install_policy: InstallPolicy,
    pub channel_overrides: &'a HashMap<String, source::InstallSource>,
    pub dry_run: bool,
}

impl Engine<'_> {
    /// Runs the pipeline for every tool, continuing past failures.
    pub fn sync_all(&mut self, tools: &[ToolSpec]) -> Vec<ToolReport> {
        tools.iter().map(|tool| self.sync_tool(tool)).collect()
    }

    pub fn sync_tool(&mut self, tool: &ToolSpec) -> ToolReport {
        match (self.find_bin)(tool.bin) {
            Some(path) => self.sync_installed(tool, &path),
            None => self.sync_missing(tool),
        }
    }

    fn version_command(tool: &ToolSpec) -> Command {
        Command::new(tool.bin, tool.version_args)
    }

    /// "Works" means `--version` actually runs (original works()); returns
    /// its first line as the version string.
    fn capture_version(&mut self, cmd: &Command) -> Option<String> {
        match self.probe.capture(cmd) {
            Ok(cap) if cap.success => Some(first_line(&cap.stdout)),
            _ => None,
        }
    }

    /// Runs commands in order via exec, stopping at the first failure.
    fn run_all(&mut self, cmds: &[Command]) -> Result<(), String> {
        for cmd in cmds {
            match self.exec.run(cmd) {
                Ok(true) => {}
                Ok(false) => return Err(format!("command failed: {cmd}")),
                Err(err) => return Err(format!("could not launch {cmd}: {err}")),
            }
        }
        Ok(())
    }

    fn sync_installed(&mut self, tool: &ToolSpec, path: &std::path::Path) -> ToolReport {
        let version_cmd = Self::version_command(tool);
        let installed_source = source::detect_from_path(path);
        let configured_source = self.channel_overrides.get(tool.id).copied();
        let update_source = configured_source.unwrap_or(installed_source);
        let mut report = ToolReport {
            id: tool.id.to_string(),
            display: tool.display.to_string(),
            installed: true,
            before: self.capture_version(&version_cmd),
            after: None,
            action: ActionKind::Update,
            outcome: Outcome::Ok,
            reason: None,
            commands: Vec::new(),
        };

        let cmds = match (tool.update)(self.os, update_source) {
            Support::Supported(cmds) => pin_npm_owner(cmds, installed_source, path),
            Support::Unsupported(reason) => {
                report.outcome = Outcome::Skip;
                report.reason = Some(match configured_source {
                    Some(source) => format!(
                        "configured channel `{}` unsupported: {reason}",
                        source.channel_name()
                    ),
                    None => reason.to_string(),
                });
                return report;
            }
        };
        report.commands = cmds.iter().map(|c| c.to_string()).collect();

        if self.dry_run {
            for cmd in &cmds {
                let _ = self.exec.run(cmd);
            }
            return report;
        }

        let run_result = self.run_all(&cmds);

        // Verification is the truth: an update command may exit nonzero when
        // already current (original-script behavior).
        if let Some(version) = self.capture_version(&version_cmd) {
            report.after = Some(version);
            if run_result.is_err() {
                report.reason = Some(
                    "update command reported failure but the tool still works \
                     (may already be current)"
                        .to_string(),
                );
            }
            return report;
        }

        if let Some(hook) = tool.on_broken {
            let recovery = pin_npm_owner(hook(self.os, installed_source), installed_source, path);
            report
                .commands
                .extend(recovery.iter().map(|c| c.to_string()));
            if self.run_all(&recovery).is_ok()
                && let Some(version) = self.capture_version(&version_cmd)
            {
                report.after = Some(version);
                report.reason = Some("recovered via reinstall after a broken state".to_string());
                return report;
            }
        }

        report.outcome = Outcome::Fail;
        report.reason = Some(match run_result {
            Err(err) => format!("verification failed after update ({err})"),
            Ok(()) => "verification failed after update (--version does not run)".to_string(),
        });
        report
    }

    fn sync_missing(&mut self, tool: &ToolSpec) -> ToolReport {
        let mut report = ToolReport {
            id: tool.id.to_string(),
            display: tool.display.to_string(),
            installed: false,
            before: None,
            after: None,
            action: ActionKind::Install,
            outcome: Outcome::Ok,
            reason: None,
            commands: Vec::new(),
        };

        let cmds = match (tool.install)(self.os) {
            Support::Supported(cmds) => cmds,
            Support::Unsupported(reason) => {
                report.outcome = Outcome::Skip;
                report.reason = Some(reason.to_string());
                return report;
            }
        };

        let proceed = match self.install_policy {
            InstallPolicy::Always => true,
            InstallPolicy::Never => {
                report.outcome = Outcome::Skip;
                report.reason =
                    Some("not installed; installation disabled (--no-install)".to_string());
                return report;
            }
            InstallPolicy::Prompt => (self.consent)(tool.display),
        };
        if !proceed {
            report.outcome = Outcome::Skip;
            report.reason = Some("installation declined".to_string());
            return report;
        }
        report.commands = cmds.iter().map(|c| c.to_string()).collect();

        if self.dry_run {
            for cmd in &cmds {
                let _ = self.exec.run(cmd);
            }
            return report;
        }

        if let Err(err) = self.run_all(&cmds) {
            report.outcome = Outcome::Fail;
            report.reason = Some(format!("install failed: {err}"));
            return report;
        }

        let version_cmd = Self::version_command(tool);
        if let Some(version) = self.capture_version(&version_cmd) {
            report.after = Some(version);
            return report;
        }

        // Fresh installs may land outside the current PATH (SPEC §5.5):
        // re-check the known install dir before giving advice.
        if let Some(dir) = (tool.install_dir)(self.os) {
            for absolute in install_dir_candidates(self.os, &dir, tool.bin) {
                let absolute_cmd = Command::new(&absolute.to_string_lossy(), tool.version_args);
                if let Some(version) = self.capture_version(&absolute_cmd) {
                    report.after = Some(version);
                    report.reason = Some(format!(
                        "installed at {}; restart your shell to refresh PATH",
                        dir.display()
                    ));
                    return report;
                }
            }
        }

        // Installed but not verifiable in this session — advice, not FAIL
        // (SPEC §5.5).
        report.reason = Some(format!(
            "installed; restart your shell and run `{} --version` to verify (PATH not refreshed)",
            tool.bin
        ));
        report
    }
}

fn install_dir_candidates(os: &OsInfo, dir: &std::path::Path, bin: &str) -> Vec<PathBuf> {
    let mut candidates = vec![dir.join(bin)];
    if os.os == Os::Windows {
        candidates.push(dir.join(format!("{bin}.exe")));
    }
    candidates
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").trim().to_string()
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

    fn windows10() -> OsInfo {
        OsInfo {
            os: Os::Windows,
            arch: "x86_64".into(),
            windows_build: Some(19045),
            libc: None,
        }
    }

    fn windows11() -> OsInfo {
        OsInfo {
            os: Os::Windows,
            arch: "x86_64".into(),
            windows_build: Some(22631),
            libc: None,
        }
    }

    /// Offline fixture tool (SPEC §8.4): native-only update, fixed install
    /// dir for the PATH-recheck behavior.
    fn footool() -> ToolSpec {
        ToolSpec {
            id: "footool",
            display: "Foo Tool",
            bin: "footool",
            version_args: &["--version"],
            install_dir: |_| Some(PathBuf::from("/fixture/bin")),
            self_updates: false,
            install: |_| Supported(vec![Command::sh("foo-install.sh")]),
            update: |_, source| match source {
                crate::source::InstallSource::Native => {
                    Supported(vec![Command::new("footool", &["update"])])
                }
                _ => Unsupported("footool is native-only"),
            },
            on_broken: None,
            latest_source: |_| crate::tools::LatestSource::SelfUpdating,
            install_script: |_| None,
        }
    }

    fn recovering_footool() -> ToolSpec {
        ToolSpec {
            on_broken: Some(|_, _| vec![Command::new("footool-reinstall", &[])]),
            ..footool()
        }
    }

    fn multi_channel_footool() -> ToolSpec {
        ToolSpec {
            update: |_, source| match source {
                crate::source::InstallSource::Native => {
                    Supported(vec![Command::new("footool", &["update"])])
                }
                crate::source::InstallSource::Brew => {
                    Supported(vec![Command::new("brew", &["upgrade", "footool"])])
                }
                crate::source::InstallSource::Npm => Supported(vec![Command::new(
                    "npm",
                    &["install", "-g", "footool@latest"],
                )]),
                _ => Unsupported("fixture channel unsupported"),
            },
            ..footool()
        }
    }

    struct Fixture {
        os: OsInfo,
        probe: MockRunner,
        exec: MockRunner,
        consent_log: Vec<String>,
        consent_answer: bool,
        install_policy: InstallPolicy,
        channel_overrides: HashMap<String, crate::source::InstallSource>,
        dry_run: bool,
    }

    impl Fixture {
        fn new() -> Fixture {
            Fixture {
                os: macos(),
                probe: MockRunner::new(),
                exec: MockRunner::new(),
                consent_log: Vec::new(),
                consent_answer: true,
                install_policy: InstallPolicy::Prompt,
                channel_overrides: HashMap::new(),
                dry_run: false,
            }
        }

        fn sync(&mut self, tool: &ToolSpec, found_at: Option<&str>) -> ToolReport {
            let found = found_at.map(PathBuf::from);
            let find_bin = move |_: &str| found.clone();
            let answer = self.consent_answer;
            let log = &mut self.consent_log;
            let mut consent = |display: &str| {
                log.push(display.to_string());
                answer
            };
            let mut engine = Engine {
                os: &self.os,
                find_bin: &find_bin,
                probe: &mut self.probe,
                exec: &mut self.exec,
                consent: &mut consent,
                install_policy: self.install_policy,
                channel_overrides: &self.channel_overrides,
                dry_run: self.dry_run,
            };
            engine.sync_tool(tool)
        }
    }

    /// npm-channel fixture for the owning-npm pin behavior. Gated like
    /// its only consumer (the unix symlink test) so Windows clippy does
    /// not flag it as dead code.
    #[cfg(unix)]
    fn npm_footool() -> ToolSpec {
        ToolSpec {
            update: |_, source| match source {
                crate::source::InstallSource::Npm => Supported(vec![Command::new(
                    "npm",
                    &["install", "-g", "footool@latest"],
                )]),
                _ => Unsupported("npm-only fixture"),
            },
            ..footool()
        }
    }

    #[cfg(unix)]
    #[test]
    fn npm_updates_are_pinned_to_the_owning_npm() {
        // nvm-style setups: a bare `npm i -g` resolves to whichever npm is
        // active and installs into that npm's own prefix — a duplicate, not
        // an update (SPEC §11, found live). The engine must run the npm
        // that owns the existing install.
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("sync-engine-npm-{}", std::process::id()));
        let store = root.join("prefix/lib/node_modules/footool/bin");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(store.join("footool.js"), "// stub").unwrap();
        let bin_dir = root.join("prefix/bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(bin_dir.join("npm"), "#!/bin/sh\n").unwrap();
        let shim = bin_dir.join("footool");
        std::os::unix::fs::symlink(store.join("footool.js"), &shim).unwrap();

        let pinned = format!(
            "{} install -g footool@latest",
            bin_dir.join("npm").display()
        );
        let mut fx = Fixture::new();
        fx.probe.script_capture("footool --version", true, "1.0");
        fx.probe.script_capture("footool --version", true, "1.1");
        let report = fx.sync(&npm_footool(), Some(shim.to_str().unwrap()));

        assert_eq!(report.outcome, Outcome::Ok);
        assert!(fx.exec.saw(&pinned), "calls: {:?}", fx.exec.calls);
        assert!(
            !fx.exec.saw("npm install -g footool@latest"),
            "a bare npm must not run: {:?}",
            fx.exec.calls
        );
        // The report (and therefore --dry-run) shows the real command.
        assert_eq!(report.commands, vec![pinned]);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn updates_installed_tool_and_records_versions() {
        let mut fx = Fixture::new();
        fx.probe.script_capture("footool --version", true, "1.0");
        fx.probe.script_capture("footool --version", true, "1.1");
        let report = fx.sync(&footool(), Some("/usr/local/bin/footool"));

        assert_eq!(report.outcome, Outcome::Ok);
        assert_eq!(report.action, ActionKind::Update);
        assert!(report.installed);
        assert_eq!(report.before.as_deref(), Some("1.0"));
        assert_eq!(report.after.as_deref(), Some("1.1"));
        assert_eq!(report.commands, vec!["footool update"]);
        assert!(fx.exec.saw("footool update"));
    }

    #[test]
    fn channel_override_replaces_detected_update_source() {
        let mut fx = Fixture::new();
        fx.channel_overrides
            .insert("footool".to_string(), crate::source::InstallSource::Npm);
        fx.probe.script_capture("footool --version", true, "1.0");
        fx.probe.script_capture("footool --version", true, "1.1");
        let report = fx.sync(
            &multi_channel_footool(),
            Some("/opt/homebrew/Cellar/footool/1.0/bin/footool"),
        );

        assert_eq!(report.outcome, Outcome::Ok);
        assert_eq!(report.commands, vec!["npm install -g footool@latest"]);
        assert!(fx.exec.saw("npm install -g footool@latest"));
        assert!(!fx.exec.saw("brew upgrade footool"));
    }

    #[test]
    fn absent_channel_override_keeps_detected_source_behavior() {
        let mut fx = Fixture::new();
        fx.probe.script_capture("footool --version", true, "1.0");
        fx.probe.script_capture("footool --version", true, "1.1");
        let report = fx.sync(
            &multi_channel_footool(),
            Some("/opt/homebrew/Cellar/footool/1.0/bin/footool"),
        );

        assert_eq!(report.outcome, Outcome::Ok);
        assert_eq!(report.commands, vec!["brew upgrade footool"]);
        assert!(fx.exec.saw("brew upgrade footool"));
    }

    #[test]
    fn unsupported_channel_override_skips_with_configured_channel_reason() {
        let mut fx = Fixture::new();
        fx.channel_overrides
            .insert("footool".to_string(), crate::source::InstallSource::Npm);
        fx.probe.script_capture("footool --version", true, "1.0");
        let report = fx.sync(&footool(), Some("/usr/local/bin/footool"));

        assert_eq!(report.outcome, Outcome::Skip);
        let reason = report.reason.expect("explains configured override");
        assert!(
            reason.contains("configured channel `npm`"),
            "reason: {reason}"
        );
        assert!(
            reason.contains("footool is native-only"),
            "reason: {reason}"
        );
        assert!(fx.exec.calls.is_empty());
    }

    #[test]
    fn channel_override_does_not_apply_to_missing_tool_installs() {
        let mut fx = Fixture::new();
        fx.install_policy = InstallPolicy::Always;
        fx.channel_overrides
            .insert("footool".to_string(), crate::source::InstallSource::Npm);
        fx.probe.script_capture("footool --version", true, "1.0");
        let report = fx.sync(&multi_channel_footool(), None);

        assert_eq!(report.action, ActionKind::Install);
        assert_eq!(report.outcome, Outcome::Ok);
        assert_eq!(report.commands, vec!["sh -c foo-install.sh".to_string()]);
    }

    #[test]
    fn dry_run_records_overridden_update_command() {
        let mut fx = Fixture::new();
        fx.dry_run = true;
        fx.channel_overrides
            .insert("footool".to_string(), crate::source::InstallSource::Npm);
        fx.probe.script_capture("footool --version", true, "1.0");
        let report = fx.sync(
            &multi_channel_footool(),
            Some("/opt/homebrew/Cellar/footool/1.0/bin/footool"),
        );

        assert_eq!(report.outcome, Outcome::Ok);
        assert_eq!(report.after, None);
        assert_eq!(report.commands, vec!["npm install -g footool@latest"]);
        assert!(fx.exec.saw("npm install -g footool@latest"));
    }

    #[test]
    fn npm_override_from_non_npm_install_falls_back_to_bare_npm() {
        let mut fx = Fixture::new();
        fx.channel_overrides
            .insert("footool".to_string(), crate::source::InstallSource::Npm);
        fx.probe.script_capture("footool --version", true, "1.0");
        fx.probe.script_capture("footool --version", true, "1.1");
        let report = fx.sync(&multi_channel_footool(), Some("/usr/local/bin/footool"));

        assert_eq!(report.outcome, Outcome::Ok);
        assert_eq!(report.commands, vec!["npm install -g footool@latest"]);
        assert!(fx.exec.saw("npm install -g footool@latest"));
    }

    #[test]
    fn update_command_failure_with_working_tool_is_ok_with_note() {
        // Original-script behavior: `brew upgrade` may exit nonzero when
        // already current — verification is the truth.
        let mut fx = Fixture::new();
        fx.exec.script_run("footool update", false);
        fx.probe.script_capture("footool --version", true, "1.0");
        fx.probe.script_capture("footool --version", true, "1.0");
        let report = fx.sync(&footool(), Some("/usr/local/bin/footool"));

        assert_eq!(report.outcome, Outcome::Ok);
        assert!(report.reason.expect("keeps a note").contains("still works"));
    }

    #[test]
    fn update_verify_failure_without_recovery_fails() {
        let mut fx = Fixture::new();
        fx.probe.script_capture("footool --version", true, "1.0");
        fx.probe.script_capture("footool --version", false, "");
        let report = fx.sync(&footool(), Some("/usr/local/bin/footool"));

        assert_eq!(report.outcome, Outcome::Fail);
        assert!(report.reason.expect("explains").contains("verification"));
        assert_eq!(report.after, None);
    }

    #[test]
    fn broken_tool_recovers_via_on_broken_hook() {
        let mut fx = Fixture::new();
        fx.probe.script_capture("footool --version", false, ""); // broken before
        fx.probe.script_capture("footool --version", false, ""); // still broken after update
        fx.probe.script_capture("footool --version", true, "2.0"); // fixed after recovery
        let report = fx.sync(&recovering_footool(), Some("/usr/local/bin/footool"));

        assert_eq!(report.outcome, Outcome::Ok);
        assert_eq!(report.before, None);
        assert_eq!(report.after.as_deref(), Some("2.0"));
        assert!(fx.exec.saw("footool-reinstall"));
        assert!(report.reason.expect("notes recovery").contains("recover"));
        assert!(report.commands.contains(&"footool-reinstall".to_string()));
    }

    #[test]
    fn unsupported_update_channel_skips_with_reason() {
        let mut fx = Fixture::new();
        fx.probe.script_capture("footool --version", true, "1.0");
        // Brew-classified path → fixture update says native-only.
        let report = fx.sync(
            &footool(),
            Some("/opt/homebrew/Cellar/footool/1.0/bin/footool"),
        );

        assert_eq!(report.outcome, Outcome::Skip);
        assert_eq!(report.reason.as_deref(), Some("footool is native-only"));
        assert!(report.installed);
        assert_eq!(report.before.as_deref(), Some("1.0"));
        assert!(fx.exec.calls.is_empty());
    }

    #[test]
    fn missing_tool_with_never_policy_skips() {
        let mut fx = Fixture::new();
        fx.install_policy = InstallPolicy::Never;
        let report = fx.sync(&footool(), None);

        assert_eq!(report.outcome, Outcome::Skip);
        assert_eq!(report.action, ActionKind::Install);
        assert!(!report.installed);
        assert!(report.reason.expect("explains").contains("--no-install"));
        assert!(fx.exec.calls.is_empty());
        assert!(fx.consent_log.is_empty());
    }

    #[test]
    fn missing_tool_prompt_declined_skips() {
        let mut fx = Fixture::new();
        fx.consent_answer = false;
        let report = fx.sync(&footool(), None);

        assert_eq!(report.outcome, Outcome::Skip);
        assert!(report.reason.expect("explains").contains("declined"));
        assert_eq!(fx.consent_log, vec!["Foo Tool"]);
        assert!(fx.exec.calls.is_empty());
    }

    #[test]
    fn missing_tool_prompt_accepted_installs_and_verifies() {
        let mut fx = Fixture::new();
        fx.probe.script_capture("footool --version", true, "1.0");
        let report = fx.sync(&footool(), None);

        assert_eq!(report.outcome, Outcome::Ok);
        assert_eq!(report.action, ActionKind::Install);
        assert_eq!(report.after.as_deref(), Some("1.0"));
        assert!(fx.exec.saw(r#"sh -c foo-install.sh"#));
        assert_eq!(fx.consent_log, vec!["Foo Tool"]);
    }

    #[test]
    fn missing_tool_with_always_policy_installs_without_prompting() {
        let mut fx = Fixture::new();
        fx.install_policy = InstallPolicy::Always;
        fx.probe.script_capture("footool --version", true, "1.0");
        let report = fx.sync(&footool(), None);

        assert_eq!(report.outcome, Outcome::Ok);
        assert!(fx.consent_log.is_empty());
    }

    /// The engine joins install_dir with the bin name using native path
    /// separators; tests must script the same rendering or they break on
    /// Windows (caught by CI on windows-latest).
    fn fixture_absolute_probe() -> String {
        format!(
            "{} --version",
            PathBuf::from("/fixture/bin")
                .join("footool")
                .to_string_lossy()
        )
    }

    fn fixture_absolute_windows_exe_probe() -> String {
        format!(
            "{} --version",
            PathBuf::from("/fixture/bin")
                .join("footool.exe")
                .to_string_lossy()
        )
    }

    #[test]
    fn fresh_install_rechecks_install_dir_when_path_not_refreshed() {
        // SPEC §5.5: a fresh install may land outside the current PATH.
        let mut fx = Fixture::new();
        fx.install_policy = InstallPolicy::Always;
        fx.probe.script_capture("footool --version", false, "");
        fx.probe
            .script_capture(&fixture_absolute_probe(), true, "1.0");
        let report = fx.sync(&footool(), None);

        assert_eq!(report.outcome, Outcome::Ok);
        assert_eq!(report.after.as_deref(), Some("1.0"));
        assert!(report.reason.expect("advises").contains("restart"));
    }

    #[test]
    fn fresh_install_rechecks_windows_exe_in_install_dir() {
        let mut fx = Fixture::new();
        fx.os = windows11();
        fx.install_policy = InstallPolicy::Always;
        fx.probe.script_capture("footool --version", false, "");
        fx.probe
            .script_capture(&fixture_absolute_probe(), false, "");
        fx.probe
            .script_capture(&fixture_absolute_windows_exe_probe(), true, "1.0");
        let report = fx.sync(&footool(), None);

        assert_eq!(report.outcome, Outcome::Ok);
        assert_eq!(report.after.as_deref(), Some("1.0"));
        assert!(report.reason.expect("advises").contains("restart"));
    }

    #[test]
    fn fresh_install_unverifiable_advises_restart_instead_of_failing() {
        // SPEC §5.5: not verifiable in this session is advice, not FAIL.
        let mut fx = Fixture::new();
        fx.install_policy = InstallPolicy::Always;
        fx.probe.script_capture("footool --version", false, "");
        fx.probe
            .script_capture(&fixture_absolute_probe(), false, "");
        let report = fx.sync(&footool(), None);

        assert_eq!(report.outcome, Outcome::Ok);
        assert_eq!(report.after, None);
        assert!(report.reason.expect("advises").contains("restart"));
    }

    #[test]
    fn failed_install_command_fails() {
        let mut fx = Fixture::new();
        fx.install_policy = InstallPolicy::Always;
        fx.exec.script_run(r#"sh -c foo-install.sh"#, false);
        let report = fx.sync(&footool(), None);

        assert_eq!(report.outcome, Outcome::Fail);
        assert!(report.reason.expect("explains").contains("install"));
    }

    #[test]
    fn unsupported_install_skips_with_reason() {
        // Registry integration: Kiro on Windows 10 (SPEC §8.1 example).
        let mut fx = Fixture::new();
        fx.os = windows10();
        let kiro = crate::tools::registry()
            .into_iter()
            .find(|t| t.id == "kiro")
            .unwrap();
        let report = fx.sync(&kiro, None);

        assert_eq!(report.outcome, Outcome::Skip);
        assert!(report.reason.expect("explains").contains("Windows 11"));
        assert!(fx.exec.calls.is_empty());
    }

    #[test]
    fn dry_run_records_plan_without_verifying() {
        let mut fx = Fixture::new();
        fx.dry_run = true;
        fx.probe.script_capture("footool --version", true, "1.0");
        let report = fx.sync(&footool(), Some("/usr/local/bin/footool"));

        assert_eq!(report.outcome, Outcome::Ok);
        assert_eq!(report.before.as_deref(), Some("1.0"));
        assert_eq!(report.after, None);
        assert!(fx.exec.saw("footool update"));
        // Only the before-probe ran — no post-verify under dry-run.
        assert_eq!(fx.probe.calls.len(), 1);
    }

    #[test]
    fn sync_all_continues_past_failures_and_aggregates() {
        let mut probe = MockRunner::new();
        let mut exec = MockRunner::new();
        // First tool: verify fails after update → Fail. Second: fine.
        probe.script_capture("footool --version", true, "1.0");
        probe.script_capture("footool --version", false, "");
        probe.script_capture("footool --version", true, "1.0");
        probe.script_capture("footool --version", true, "1.1");
        let found = PathBuf::from("/usr/local/bin/footool");
        let find_bin = move |_: &str| Some(found.clone());
        let mut consent = |_: &str| true;
        let os = macos();
        let channel_overrides = HashMap::new();
        let mut engine = Engine {
            os: &os,
            find_bin: &find_bin,
            probe: &mut probe,
            exec: &mut exec,
            consent: &mut consent,
            install_policy: InstallPolicy::Prompt,
            channel_overrides: &channel_overrides,
            dry_run: false,
        };

        let reports = engine.sync_all(&[footool(), footool()]);
        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].outcome, Outcome::Fail);
        assert_eq!(reports[1].outcome, Outcome::Ok);
        assert!(!all_ok(&reports));

        let ok_only = vec![reports[1].clone()];
        assert!(all_ok(&ok_only));
        // Skips never fail the run.
        let mut skip = reports[1].clone();
        skip.outcome = Outcome::Skip;
        assert!(all_ok(&[skip]));
    }
}
