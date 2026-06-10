//! Gemini CLI, Google (SPEC §7.3). No native self-update — managed through
//! npm (default, universal) or brew when installed that way. Node 20+ is a
//! prerequisite of the npm channel (engine surfaces a clear SKIP when npm is
//! absent, SPEC §5.5).

use std::path::PathBuf;

use crate::os::{Os, OsInfo};
use crate::runner::Command;
use crate::source::InstallSource;
use crate::tools::{Support, ToolSpec};

pub fn spec() -> ToolSpec {
    ToolSpec {
        id: "gemini",
        display: "Gemini CLI",
        bin: "gemini",
        version_args: &["--version"],
        install_dir,
        self_updates: false,
        install,
        update,
        on_broken: None,
    }
}

fn install_dir(_os: &OsInfo) -> Option<PathBuf> {
    None // npm-managed; global bin dir varies with the Node setup
}

fn install(_os: &OsInfo) -> Support<Vec<Command>> {
    Support::Supported(vec![Command::new(
        "npm",
        &["install", "-g", "@google/gemini-cli"],
    )])
}

fn update(os: &OsInfo, source: InstallSource) -> Support<Vec<Command>> {
    match source {
        InstallSource::Npm => Support::Supported(vec![Command::new(
            "npm",
            &["install", "-g", "@google/gemini-cli@latest"],
        )]),
        InstallSource::Brew => match os.os {
            Os::MacOs | Os::Linux => {
                Support::Supported(vec![Command::new("brew", &["upgrade", "gemini-cli"])])
            }
            Os::Windows => Support::Unsupported("brew is not available on Windows"),
        },
        InstallSource::Native => Support::Unsupported(
            "Gemini has no native self-update; managed via npm or brew (SPEC §7.3)",
        ),
        InstallSource::Winget | InstallSource::Scoop => {
            Support::Unsupported("Gemini is not distributed via winget/Scoop (SPEC §7.3)")
        }
    }
}
