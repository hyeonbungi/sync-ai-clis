//! Binary entrypoint: parses flags, loads config, wires the real effects
//! (PATH lookup, runners, stdin consent) into the engine, and renders the
//! report. Exit codes per SPEC §6.3: 0 all OK, 1 any failure, 2 usage error.

use std::io::Write;

use clap::Parser;

use sync_ai_clis::audit;
use sync_ai_clis::baseline::BaselineStore;
use sync_ai_clis::check;
use sync_ai_clis::cli::{self, Cli, Subcmd};
use sync_ai_clis::config;
use sync_ai_clis::doctor;
use sync_ai_clis::engine::{self, Engine};
use sync_ai_clis::os::OsInfo;
use sync_ai_clis::report;
use sync_ai_clis::runner::{Command, CommandRunner, DryRunRunner, RealRunner};
use sync_ai_clis::source;
use sync_ai_clis::tools::{ToolSpec, registry};

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let cli = Cli::parse(); // usage errors exit 2 via clap

    let config = match config::load() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("error: {err}");
            return 2;
        }
    };
    let registry = registry();
    let channel_overrides = match config::channel_overrides(config.channels.as_ref(), &registry) {
        Ok(overrides) => overrides,
        Err(err) => {
            eprintln!("error: {err}");
            return 2;
        }
    };
    let tools = match cli::select_tools(registry, &cli.only, &cli.except, config.tools.as_deref()) {
        Ok(tools) => tools,
        Err(err) => {
            eprintln!("error: {err}");
            return 2;
        }
    };
    let Some(os) = OsInfo::detect() else {
        eprintln!("error: unsupported platform `{}`", std::env::consts::OS);
        return 1;
    };

    // Exhaustive on purpose: a new subcommand must be wired here to compile.
    match cli.command {
        Some(Subcmd::List) => return list(&tools),
        Some(Subcmd::Doctor) => return doctor_cmd(&tools, &os, cli.json),
        Some(Subcmd::Check) => return check_cmd(&tools, &os, cli.json),
        Some(Subcmd::Audit { accept }) => return audit_cmd(&tools, &os, cli.json, accept),
        None => {}
    }

    let policy = cli::install_policy(cli.yes, cli.no_install, &config);
    let find_bin = |bin: &str| source::find_in_path(bin);
    // Read-only probes stay real even under --dry-run (see runner.rs).
    let mut probe = RealRunner::interactive();
    // --json keeps stdout machine-pure: child output routes to stderr.
    let mut real_exec = if cli.json {
        RealRunner::quiet()
    } else {
        RealRunner::interactive()
    };
    let mut dry_exec = DryRunRunner::new();
    let exec: &mut dyn CommandRunner = if cli.dry_run {
        &mut dry_exec
    } else {
        &mut real_exec
    };
    let mut consent = prompt_install;
    let mut engine = Engine {
        os: &os,
        find_bin: &find_bin,
        probe: &mut probe,
        exec,
        consent: &mut consent,
        install_policy: policy,
        channel_overrides: &channel_overrides,
        dry_run: cli.dry_run,
    };

    let quiet = cli.json; // keep stdout pure JSON for automation
    let mut reports = Vec::new();
    for tool in &tools {
        if !quiet {
            anstream::println!("\n==> {}", tool.display);
        }
        let report = engine.sync_tool(tool);
        if !quiet {
            anstream::println!("   {}", report::tool_line(&report, true, cli.dry_run));
            if cli.dry_run {
                for command in &report.commands {
                    anstream::println!("   would run: {command}");
                }
            }
        }
        reports.push(report);
    }

    if cli.json {
        println!("{}", report::json_summary(&reports));
    } else {
        anstream::println!("\n==> Summary");
        anstream::println!("{}", report::summary_table(&reports, true, cli.dry_run));
    }
    if engine::all_ok(&reports) { 0 } else { 1 }
}

/// `doctor`: read-only diagnosis of broken/duplicate/shadowed installs
/// (SPEC §6.1/§6.3). Exit 1 when issues are found, 0 when clean.
fn doctor_cmd(tools: &[ToolSpec], os: &OsInfo, json: bool) -> i32 {
    let mut probe = RealRunner::interactive();
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    let diagnoses: Vec<_> = tools
        .iter()
        .map(|tool| doctor::diagnose(tool, os, &path_var, &mut probe))
        .collect();

    if json {
        println!("{}", doctor::json_doctor(&diagnoses));
    } else {
        for diagnosis in &diagnoses {
            anstream::println!("\n{}", doctor::render(diagnosis));
        }
        anstream::println!(
            "\n{}",
            if doctor::has_issues(&diagnoses) {
                "Problems found — see the advice lines above."
            } else {
                "No problems found."
            }
        );
    }
    if doctor::has_issues(&diagnoses) { 1 } else { 0 }
}

/// `check`: read-only "is an update available?" per tool (design doc 0012).
/// Probes installed `--version` and each tool's declared latest source
/// (`npm view` / official manifest); changes nothing. Exit 10 when any update
/// is available, 1 when a verdict is inconclusive, 0 when all current.
fn check_cmd(tools: &[ToolSpec], os: &OsInfo, json: bool) -> i32 {
    let find_bin = |bin: &str| source::find_in_path(bin);
    let mut probe = RealRunner::interactive();
    let results: Vec<_> = tools
        .iter()
        .map(|tool| check::check_tool(tool, os, &find_bin, &mut probe))
        .collect();

    if json {
        println!("{}", check::json_check(&results));
    } else {
        anstream::println!("{}", check::render(&results));
    }
    check::exit_code(&results)
}

/// `audit`: read-only detection of changes in remote install scripts (design
/// doc 0013). Fetches each tool's install script and compares it to the
/// accepted baseline; `--accept` records the current scripts as the new
/// baseline (the only write). Exit 10 when any changed, 1 when a fetch failed,
/// 0 otherwise.
fn audit_cmd(tools: &[ToolSpec], os: &OsInfo, json: bool, accept: bool) -> i32 {
    let Some(dir) = BaselineStore::default_dir() else {
        eprintln!("error: no data directory for script baselines on this platform");
        return 1;
    };
    let store = BaselineStore::new(dir);
    let mut fetch = RealRunner::interactive();
    let results: Vec<_> = tools
        .iter()
        .map(|tool| {
            if accept {
                audit::accept_tool(tool, os, &mut fetch, &store)
            } else {
                audit::audit_tool(tool, os, &mut fetch, &store)
            }
        })
        .collect();

    if json {
        println!("{}", audit::json_audit(&results));
    } else {
        anstream::println!("{}", audit::render(&results));
    }
    audit::exit_code(&results)
}

/// `list` / `status`: read-only table of known tools (SPEC §6.1).
fn list(tools: &[ToolSpec]) -> i32 {
    let mut probe = RealRunner::interactive();
    anstream::println!("{:<18} {:<10} {}", "TOOL", "INSTALLED", "VERSION");
    for tool in tools {
        let found = source::find_in_path(tool.bin);
        let version = found.as_ref().and_then(|_| {
            match probe.capture(&Command::new(tool.bin, tool.version_args)) {
                Ok(cap) if cap.success => Some(cap.stdout.lines().next().unwrap_or("").to_string()),
                _ => None,
            }
        });
        anstream::println!(
            "{:<18} {:<10} {}",
            tool.display,
            if found.is_some() { "yes" } else { "no" },
            version.as_deref().unwrap_or("-")
        );
    }
    0
}

fn prompt_install(display: &str) -> bool {
    print!("{display} is not installed. Install it? (y/N) ");
    std::io::stdout().flush().ok();
    let mut answer = String::new();
    if std::io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}
