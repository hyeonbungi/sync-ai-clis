//! Codex CLI, OpenAI (SPEC §7.2). No native self-update subcommand confirmed
//! yet (SPEC §11 backlog) — native updates rerun the official installer.
//! Keeps the original script's "reinstall when broken" recovery. The npm
//! package must be the scoped @openai/codex (the unscoped `codex` package is
//! an unrelated 2012 project).

use std::path::PathBuf;

use crate::os::{Os, OsInfo};
use crate::runner::Command;
use crate::source::InstallSource;
use crate::tools::{Support, ToolSpec};

const INSTALL_SH_URL: &str = "https://chatgpt.com/codex/install.sh";
const INSTALL_PS1_URL: &str = "https://chatgpt.com/codex/install.ps1";

pub fn spec() -> ToolSpec {
    ToolSpec {
        id: "codex",
        display: "Codex CLI",
        bin: "codex",
        version_args: &["--version"],
        install_dir,
        self_updates: false,
        install,
        update,
        on_broken: Some(on_broken),
    }
}

fn install_dir(_os: &OsInfo) -> Option<PathBuf> {
    None // not confirmed yet (SPEC §11 backlog: install_dir 확정)
}

fn install(os: &OsInfo) -> Support<Vec<Command>> {
    Support::Supported(vec![match os.os {
        Os::MacOs | Os::Linux => Command::sh(&format!("curl -fsSL {INSTALL_SH_URL} | sh")),
        Os::Windows => Command::powershell(&format!("irm {INSTALL_PS1_URL} | iex")),
    }])
}

fn update(os: &OsInfo, source: InstallSource) -> Support<Vec<Command>> {
    match source {
        // Installer rerun until a self-update subcommand is confirmed (SPEC §7.2 TODO).
        InstallSource::Native => install(os),
        InstallSource::Brew => match os.os {
            Os::MacOs => {
                Support::Supported(vec![Command::new("brew", &["upgrade", "--cask", "codex"])])
            }
            _ => Support::Unsupported("Codex brew cask is macOS-only (SPEC §7.2)"),
        },
        InstallSource::Npm => Support::Supported(vec![Command::new(
            "npm",
            &["install", "-g", "@openai/codex@latest"],
        )]),
        InstallSource::Winget | InstallSource::Scoop => {
            Support::Unsupported("Codex is not distributed via winget/Scoop (SPEC §7.2)")
        }
    }
}

/// Inherited from the original script: broken binary → reinstall through the
/// channel it came from.
fn on_broken(os: &OsInfo, source: InstallSource) -> Vec<Command> {
    match source {
        InstallSource::Brew if os.os == Os::MacOs => {
            vec![Command::new("brew", &["reinstall", "--cask", "codex"])]
        }
        _ => install(os).supported().unwrap_or_default(),
    }
}
