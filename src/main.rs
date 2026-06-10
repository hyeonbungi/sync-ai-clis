//! Binary entrypoint: parses flags, loads config, wires the real effects
//! (PATH lookup, runners, stdin consent) into the engine, and renders the
//! report. Exit codes per SPEC §6.3: 0 all OK, 1 any failure, 2 usage error.

use std::io::Write;

use clap::Parser;

use sync_ai_clis::cli::{self, Cli, Subcmd};
use sync_ai_clis::config;
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
    let tools = match cli::select_tools(registry(), &cli.only, &cli.except, config.tools.as_deref())
    {
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

    if let Some(Subcmd::List) = cli.command {
        return list(&tools);
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
