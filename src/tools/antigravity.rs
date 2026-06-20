//! Antigravity CLI, Google (SPEC §7.5). Native self-update (`agy update`),
//! no package-manager channels. Install locations are confirmed:
//! `~/.local/bin/agy` on unix, `%LOCALAPPDATA%\agy\bin` on Windows — used
//! for the PATH-not-yet-refreshed re-check after a fresh install (SPEC §5.5).

use std::path::PathBuf;

use crate::os::{Libc, Os, OsInfo};
use crate::runner::Command;
use crate::source::InstallSource;
use crate::tools::{Extract, LatestSource, Support, ToolSpec};

const INSTALL_SH_URL: &str = "https://antigravity.google/cli/install.sh";
const INSTALL_PS1_URL: &str = "https://antigravity.google/cli/install.ps1";
/// Official auto-updater host the installer itself queries (design doc 0012);
/// the per-platform manifest's `version` key is the read-only latest source.
const MANIFEST_BASE: &str = "https://antigravity-cli-auto-updater-974169037036.us-central1.run.app";

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
        latest_source,
        install_script,
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
        Os::Windows => Command::powershell_installer(&format!("irm {INSTALL_PS1_URL} | iex")),
    }])
}

fn update(_os: &OsInfo, source: InstallSource) -> Support<Vec<Command>> {
    match source {
        InstallSource::Native => Support::Supported(vec![Command::new("agy", &["update"])]),
        _ => Support::Unsupported("Antigravity has no package-manager channel (SPEC §7.5)"),
    }
}

fn latest_source(os: &OsInfo) -> LatestSource {
    // Read the same official per-platform manifest the installer uses; its
    // `version` key is the latest available release (design doc 0012). A
    // wrong/unknown platform 404s and degrades to `unknown` at runtime.
    let arch = match os.arch.as_str() {
        "x86_64" | "amd64" => "amd64",
        "aarch64" | "arm64" => "arm64",
        other => other,
    };
    let platform = match os.os {
        Os::MacOs => format!("darwin_{arch}"),
        Os::Linux if matches!(os.libc, Some(Libc::Musl)) => format!("linux_{arch}_musl"),
        Os::Linux => format!("linux_{arch}"),
        Os::Windows => format!("windows_{arch}"),
    };
    let url = format!("{MANIFEST_BASE}/manifests/{platform}.json");
    LatestSource::Probe {
        command: Command::new("curl", &["-fsSL", &url]),
        extract: Extract::JsonKey("version"),
    }
}

fn install_script(os: &OsInfo) -> Option<&'static str> {
    Some(match os.os {
        Os::MacOs | Os::Linux => INSTALL_SH_URL,
        Os::Windows => INSTALL_PS1_URL,
    })
}
