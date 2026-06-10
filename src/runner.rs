//! Command construction and execution (SPEC §5.1). The engine composes
//! [`Command`]s as data and delegates execution to a [`CommandRunner`]:
//! `RealRunner` executes, `MockRunner` records and scripts outcomes for
//! tests, `DryRunRunner` only records what would run. This split is what
//! makes OS×state command selection testable anywhere and gives `--dry-run`
//! for free.

use std::fmt;
use std::io;

/// A command as data: program + argv. Built by ToolSpecs, executed only via
/// a [`CommandRunner`]. Execution uses the argv array directly (no shell
/// interpolation); [`fmt::Display`] rendering is for humans (dry-run output,
/// logs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
}

impl Command {
    pub fn new(program: &str, args: &[&str]) -> Command {
        Command {
            program: program.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
        }
    }

    /// POSIX shell invocation for `curl … | bash` style installers:
    /// `sh -c "<script>"`.
    pub fn sh(script: &str) -> Command {
        Command::new("sh", &["-c", script])
    }

    /// Windows PowerShell invocation wrapped per SPEC §5.5:
    /// `powershell -NoProfile -ExecutionPolicy Bypass -Command "<script>"`
    /// (Windows PowerShell 5 compatible).
    pub fn powershell(script: &str) -> Command {
        Command::new(
            "powershell",
            &[
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ],
        )
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.program)?;
        for arg in &self.args {
            if arg.chars().any(char::is_whitespace) || arg.is_empty() {
                write!(f, " \"{arg}\"")?;
            } else {
                write!(f, " {arg}")?;
            }
        }
        Ok(())
    }
}

/// Result of a capturing run (e.g. `claude --version`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureResult {
    pub success: bool,
    /// Trimmed stdout.
    pub stdout: String,
}

pub trait CommandRunner {
    /// Runs interactively (stdio inherited) — installers and updaters may
    /// prompt or stream progress. Ok(true) on exit code 0.
    fn run(&mut self, cmd: &Command) -> io::Result<bool>;

    /// Runs capturing stdout (read-only probes like `--version`).
    fn capture(&mut self, cmd: &Command) -> io::Result<CaptureResult>;
}

/// Executes commands for real. `interactive()` streams child stdio to the
/// terminal (installers may prompt or show progress); `quiet()` captures
/// child output and forwards it to stderr so the process's own stdout stays
/// machine-pure — required by --json mode (SPEC §6.3).
pub struct RealRunner {
    quiet: bool,
}

impl RealRunner {
    pub fn interactive() -> RealRunner {
        RealRunner { quiet: false }
    }

    pub fn quiet() -> RealRunner {
        RealRunner { quiet: true }
    }
}

impl CommandRunner for RealRunner {
    fn run(&mut self, cmd: &Command) -> io::Result<bool> {
        if self.quiet {
            let out = std::process::Command::new(&cmd.program)
                .args(&cmd.args)
                .output()?;
            let mut stderr = io::stderr();
            let _ = io::Write::write_all(&mut stderr, &out.stdout);
            let _ = io::Write::write_all(&mut stderr, &out.stderr);
            return Ok(out.status.success());
        }
        let status = std::process::Command::new(&cmd.program)
            .args(&cmd.args)
            .status()?;
        Ok(status.success())
    }

    fn capture(&mut self, cmd: &Command) -> io::Result<CaptureResult> {
        let out = std::process::Command::new(&cmd.program)
            .args(&cmd.args)
            .output()?;
        Ok(CaptureResult {
            success: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).trim().to_string(),
        })
    }
}

/// Records every call and replays scripted outcomes FIFO per matching
/// command (so before/after `--version` sequences are scriptable);
/// unscripted or drained commands succeed (run → true, capture → success
/// with empty stdout).
#[derive(Default)]
pub struct MockRunner {
    /// Every command passed to run() or capture(), in order, rendered.
    pub calls: Vec<String>,
    scripted_runs: Vec<(String, bool)>,
    scripted_captures: Vec<(String, CaptureResult)>,
}

impl MockRunner {
    pub fn new() -> MockRunner {
        MockRunner::default()
    }

    /// Scripts the result of run() for the command rendering exactly `rendered`.
    pub fn script_run(&mut self, rendered: &str, success: bool) {
        self.scripted_runs.push((rendered.to_string(), success));
    }

    /// Scripts the result of capture() for the command rendering exactly `rendered`.
    pub fn script_capture(&mut self, rendered: &str, success: bool, stdout: &str) {
        self.scripted_captures.push((
            rendered.to_string(),
            CaptureResult {
                success,
                stdout: stdout.to_string(),
            },
        ));
    }

    /// True if a command rendering exactly `rendered` was run or captured.
    pub fn saw(&self, rendered: &str) -> bool {
        self.calls.iter().any(|c| c == rendered)
    }
}

impl CommandRunner for MockRunner {
    fn run(&mut self, cmd: &Command) -> io::Result<bool> {
        let rendered = cmd.to_string();
        self.calls.push(rendered.clone());
        let success = match self.scripted_runs.iter().position(|(r, _)| *r == rendered) {
            Some(index) => self.scripted_runs.remove(index).1,
            None => true,
        };
        Ok(success)
    }

    fn capture(&mut self, cmd: &Command) -> io::Result<CaptureResult> {
        let rendered = cmd.to_string();
        self.calls.push(rendered.clone());
        let result = match self
            .scripted_captures
            .iter()
            .position(|(r, _)| *r == rendered)
        {
            Some(index) => self.scripted_captures.remove(index).1,
            None => CaptureResult {
                success: true,
                stdout: String::new(),
            },
        };
        Ok(result)
    }
}

/// Records what would run, executes nothing (SPEC §6.1 --dry-run: print the
/// exact commands, run nothing).
#[derive(Default)]
pub struct DryRunRunner {
    /// Rendered commands that would have been executed, in order.
    pub printed: Vec<String>,
}

impl DryRunRunner {
    pub fn new() -> DryRunRunner {
        DryRunRunner::default()
    }
}

impl CommandRunner for DryRunRunner {
    /// Records the command and pretends success so the plan continues.
    fn run(&mut self, cmd: &Command) -> io::Result<bool> {
        self.printed.push(cmd.to_string());
        Ok(true)
    }

    /// Never claims state: the engine probes read-only detection with a real
    /// runner even under --dry-run, so this conservative fallback reports
    /// failure and records nothing.
    fn capture(&mut self, cmd: &Command) -> io::Result<CaptureResult> {
        let _ = cmd;
        Ok(CaptureResult {
            success: false,
            stdout: String::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_args_and_quotes_whitespace() {
        let plain = Command::new("brew", &["upgrade", "--cask", "codex"]);
        assert_eq!(plain.to_string(), "brew upgrade --cask codex");

        let piped = Command::sh("curl -fsSL https://claude.ai/install.sh | bash");
        assert_eq!(
            piped.to_string(),
            r#"sh -c "curl -fsSL https://claude.ai/install.sh | bash""#
        );
    }

    #[test]
    fn sh_wraps_script_in_dash_c() {
        let cmd = Command::sh("echo hi");
        assert_eq!(cmd.program, "sh");
        assert_eq!(cmd.args, vec!["-c", "echo hi"]);
    }

    #[test]
    fn powershell_wraps_script_per_spec_5_5() {
        let cmd = Command::powershell("irm https://claude.ai/install.ps1 | iex");
        assert_eq!(cmd.program, "powershell");
        assert_eq!(
            cmd.args,
            vec![
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                "irm https://claude.ai/install.ps1 | iex",
            ]
        );
    }

    #[test]
    fn mock_records_calls_in_order() {
        let mut mock = MockRunner::new();
        mock.run(&Command::new("claude", &["update"])).unwrap();
        mock.capture(&Command::new("claude", &["--version"]))
            .unwrap();
        assert_eq!(mock.calls, vec!["claude update", "claude --version"]);
        assert!(mock.saw("claude update"));
        assert!(!mock.saw("gemini --version"));
    }

    #[test]
    fn mock_unscripted_commands_succeed() {
        let mut mock = MockRunner::new();
        assert!(mock.run(&Command::new("claude", &["update"])).unwrap());
        let cap = mock
            .capture(&Command::new("claude", &["--version"]))
            .unwrap();
        assert!(cap.success);
        assert_eq!(cap.stdout, "");
    }

    #[test]
    fn mock_scripted_run_outcome_is_replayed() {
        let mut mock = MockRunner::new();
        mock.script_run("brew upgrade gemini-cli", false);
        assert!(
            !mock
                .run(&Command::new("brew", &["upgrade", "gemini-cli"]))
                .unwrap()
        );
        assert!(mock.run(&Command::new("claude", &["update"])).unwrap());
    }

    #[test]
    fn mock_scripted_capture_is_replayed() {
        let mut mock = MockRunner::new();
        mock.script_capture("claude --version", true, "1.2.3 (Claude Code)");
        let cap = mock
            .capture(&Command::new("claude", &["--version"]))
            .unwrap();
        assert_eq!(
            cap,
            CaptureResult {
                success: true,
                stdout: "1.2.3 (Claude Code)".into()
            }
        );
    }

    #[test]
    fn dry_run_records_would_run_commands_and_executes_nothing() {
        let mut dry = DryRunRunner::new();
        assert!(
            dry.run(&Command::sh("curl -fsSL https://x | bash"))
                .unwrap()
        );
        assert_eq!(dry.printed, vec![r#"sh -c "curl -fsSL https://x | bash""#]);

        let cap = dry
            .capture(&Command::new("claude", &["--version"]))
            .unwrap();
        assert!(!cap.success); // never claims state
        assert_eq!(dry.printed.len(), 1); // capture not recorded as "would run"
    }

    #[cfg(unix)]
    #[test]
    fn real_runner_captures_stdout_and_exit_status() {
        let mut real = RealRunner::interactive();
        let cap = real.capture(&Command::new("echo", &["hi"])).unwrap();
        assert!(cap.success);
        assert_eq!(cap.stdout, "hi");

        assert!(real.run(&Command::sh("exit 0")).unwrap());
        assert!(!real.run(&Command::sh("exit 3")).unwrap());
    }
}

#[cfg(test)]
mod ordering_tests {
    use super::*;

    #[test]
    fn mock_scripted_results_are_consumed_in_order() {
        // The engine captures --version before and after an update; scripted
        // results must replay FIFO per matching command, then fall back to
        // the unscripted default.
        let mut mock = MockRunner::new();
        mock.script_capture("claude --version", true, "1.0.0");
        mock.script_capture("claude --version", true, "2.0.0");
        mock.script_run("claude update", false);
        mock.script_run("claude update", true);

        let version = Command::new("claude", &["--version"]);
        let update = Command::new("claude", &["update"]);
        assert_eq!(mock.capture(&version).unwrap().stdout, "1.0.0");
        assert_eq!(mock.capture(&version).unwrap().stdout, "2.0.0");
        assert_eq!(mock.capture(&version).unwrap().stdout, ""); // queue drained
        assert!(!mock.run(&update).unwrap());
        assert!(mock.run(&update).unwrap());
        assert!(mock.run(&update).unwrap()); // drained → default success
    }
}

#[cfg(test)]
#[cfg(unix)]
mod quiet_tests {
    use super::*;

    #[test]
    fn quiet_real_runner_reports_status_without_touching_stdout() {
        // --json mode (SPEC §6.3) needs machine-pure stdout: child output is
        // forwarded to stderr instead. Status reporting must be identical to
        // interactive mode.
        let mut quiet = RealRunner::quiet();
        assert!(quiet.run(&Command::sh("echo to-stderr; exit 0")).unwrap());
        assert!(!quiet.run(&Command::sh("exit 3")).unwrap());

        // capture() is unaffected by quiet (it already owns the streams).
        let cap = quiet.capture(&Command::new("echo", &["hi"])).unwrap();
        assert!(cap.success);
        assert_eq!(cap.stdout, "hi");
    }
}
