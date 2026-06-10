//! Antigravity CLI, Google (SPEC §7.5). Native self-update (`agy update`),
//! no package-manager channels. Install locations are confirmed:
//! `~/.local/bin/agy` on unix, `%LOCALAPPDATA%\agy\bin` on Windows — used
//! for the PATH-not-yet-refreshed re-check after a fresh install (SPEC §5.5).

use std::path::PathBuf;

use crate::os::{Os, OsInfo};
use crate::runner::Command;
use crate::source::InstallSource;
use crate::tools::{Support, ToolSpec};

const INSTALL_SH_URL: &str = "https://antigravity.google/cli/install.sh";
const INSTALL_PS1_URL: &str = "https://antigravity.google/cli/install.ps1";

pub fn spec() -> ToolSpec {
    ToolSpec {
        id: "antigravity",
        display: "Antigravity CLI",
        bin: "agy",
        version_args: &["--version"],
        install_dir,
        self_updates: false,
        install,
        update,
        on_broken: None,
    }
}

fn install_dir(os: &OsInfo) -> Option<PathBuf> {
    match os.os {
        Os::MacOs | Os::Linux => dirs::home_dir().map(|home| home.join(".local").join("bin")),
        Os::Windows => {
            std::env::var_os("LOCALAPPDATA").map(|base| PathBuf::from(base).join("agy").join("bin"))
        }
    }
}

fn install(os: &OsInfo) -> Support<Vec<Command>> {
    Support::Supported(vec![match os.os {
        Os::MacOs | Os::Linux => Command::sh(&format!("curl -fsSL {INSTALL_SH_URL} | bash")),
        Os::Windows => Command::powershell(&format!("irm {INSTALL_PS1_URL} | iex")),
    }])
}

fn update(_os: &OsInfo, source: InstallSource) -> Support<Vec<Command>> {
    match source {
        InstallSource::Native => Support::Supported(vec![Command::new("agy", &["update"])]),
        _ => Support::Unsupported("Antigravity has no package-manager channel (SPEC §7.5)"),
    }
}
