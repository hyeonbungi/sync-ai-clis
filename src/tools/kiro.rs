//! Kiro CLI, AWS (SPEC §7.4). Native self-update (`kiro-cli update
//! --non-interactive`, background auto-apply). No package-manager channels.
//! Windows requires Win11 and uses the official PowerShell installer.
//! The official Unix installer handles the glibc≥2.34 vs musl variant on
//! Linux.

use std::path::PathBuf;

use crate::os::{Os, OsInfo};
use crate::runner::Command;
use crate::source::InstallSource;
use crate::tools::{LatestSource, Support, ToolSpec};

const INSTALL_URL: &str = "https://cli.kiro.dev/install";
const INSTALL_PS1_URL: &str = "https://cli.kiro.dev/install.ps1";

pub fn spec() -> ToolSpec {
    ToolSpec {
        id: "kiro",
        display: "Kiro CLI",
        bin: "kiro-cli",
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

fn install_dir(os: &OsInfo) -> Option<PathBuf> {
    match os.os {
        Os::MacOs => Some(
            PathBuf::from("/Applications")
                .join("Kiro CLI.app")
                .join("Contents")
                .join("MacOS"),
        ),
        Os::Linux => dirs::home_dir().map(|home| home.join(".local").join("bin")),
        Os::Windows => Some(
            std::env::var_os("ProgramFiles")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(r"C:\Program Files"))
                .join("Kiro-Cli"),
        ),
    }
}

fn install(os: &OsInfo) -> Support<Vec<Command>> {
    match os.os {
        Os::MacOs | Os::Linux => Support::Supported(vec![Command::sh(&format!(
            "curl -fsSL {INSTALL_URL} | bash"
        ))]),
        Os::Windows if !os.is_windows_11() => {
            Support::Unsupported("Kiro requires Windows 11 (SPEC §7.4)")
        }
        Os::Windows => Support::Supported(vec![Command::powershell(&format!(
            "irm {INSTALL_PS1_URL} | iex"
        ))]),
    }
}

fn update(_os: &OsInfo, source: InstallSource) -> Support<Vec<Command>> {
    match source {
        InstallSource::Native => Support::Supported(vec![Command::new(
            "kiro-cli",
            &["update", "--non-interactive"],
        )]),
        _ => Support::Unsupported("Kiro has no package-manager channel (SPEC §7.4)"),
    }
}

fn latest_source(_os: &OsInfo) -> LatestSource {
    // Kiro background-self-updates (auto-apply), so a "behind" check is not
    // meaningful — report it as self-updating (design doc 0012).
    LatestSource::SelfUpdating
}

fn install_script(os: &OsInfo) -> Option<&'static str> {
    match os.os {
        Os::MacOs | Os::Linux => Some(INSTALL_URL),
        Os::Windows if os.is_windows_11() => Some(INSTALL_PS1_URL),
        Os::Windows => None, // Win10 unsupported (no install), nothing to audit
    }
}
