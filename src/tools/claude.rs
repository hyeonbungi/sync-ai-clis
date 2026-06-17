//! Claude Code (SPEC §7.1). Native self-update (`claude update`, background
//! auto-updates) plus brew/npm/winget channels when installed that way.

use std::path::PathBuf;

use crate::os::{Os, OsInfo};
use crate::runner::Command;
use crate::source::InstallSource;
use crate::tools::{Extract, LatestSource, Support, ToolSpec};

const INSTALL_SH_URL: &str = "https://claude.ai/install.sh";
const INSTALL_PS1_URL: &str = "https://claude.ai/install.ps1";

pub fn spec() -> ToolSpec {
    ToolSpec {
        id: "claude",
        display: "Claude Code",
        bin: "claude",
        version_args: &["--version"],
        install_dir,
        self_updates: true,
        install,
        update,
        on_broken: None,
        latest_source,
        install_script,
    }
}

fn install_dir(_os: &OsInfo) -> Option<PathBuf> {
    // CI-confirmed (integration run 27285726406): the official installer
    // places claude in ~/.local/bin on unix and %USERPROFILE%\.local\bin on
    // Windows — the same home-relative dir on every OS.
    dirs::home_dir().map(|home| home.join(".local").join("bin"))
}

fn install(os: &OsInfo) -> Support<Vec<Command>> {
    Support::Supported(vec![match os.os {
        Os::MacOs | Os::Linux => Command::sh(&format!("curl -fsSL {INSTALL_SH_URL} | bash")),
        Os::Windows => Command::powershell(&format!("irm {INSTALL_PS1_URL} | iex")),
    }])
}

fn update(_os: &OsInfo, source: InstallSource) -> Support<Vec<Command>> {
    match source {
        InstallSource::Native => Support::Supported(vec![Command::new("claude", &["update"])]),
        InstallSource::Brew => {
            Support::Supported(vec![Command::new("brew", &["upgrade", "claude-code"])])
        }
        InstallSource::Npm => Support::Supported(vec![Command::new(
            "npm",
            &["install", "-g", "@anthropic-ai/claude-code@latest"],
        )]),
        InstallSource::Winget => Support::Supported(vec![Command::new(
            "winget",
            &["upgrade", "Anthropic.ClaudeCode"],
        )]),
        InstallSource::Scoop => {
            Support::Unsupported("no confirmed Scoop package for Claude Code (SPEC §7.1)")
        }
    }
}

fn latest_source(_os: &OsInfo) -> LatestSource {
    // npm is the channel-independent version oracle (design doc 0012): the
    // package tracks the same release train as the native installer, so
    // `npm view` reports latest even when claude was installed natively.
    LatestSource::Probe {
        command: Command::new("npm", &["view", "@anthropic-ai/claude-code", "version"]),
        extract: Extract::Raw,
    }
}

fn install_script(os: &OsInfo) -> Option<&'static str> {
    Some(match os.os {
        Os::MacOs | Os::Linux => INSTALL_SH_URL,
        Os::Windows => INSTALL_PS1_URL,
    })
}
