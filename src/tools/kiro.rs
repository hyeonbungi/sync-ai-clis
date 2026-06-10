//! Kiro CLI, AWS (SPEC §7.4). Native self-update (`kiro-cli update`,
//! background auto-apply). No package-manager channels. Windows requires
//! Win11, and the exact Windows install command is still unconfirmed
//! (SPEC §11 backlog) — we skip with a reason rather than guess a URL
//! (SPEC §5.5: hardcoded verified URLs only). The official installer handles
//! the glibc≥2.34 vs musl variant on Linux.

use std::path::PathBuf;

use crate::os::{Os, OsInfo};
use crate::runner::Command;
use crate::source::InstallSource;
use crate::tools::{Support, ToolSpec};

const INSTALL_URL: &str = "https://cli.kiro.dev/install";

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
    }
}

fn install_dir(_os: &OsInfo) -> Option<PathBuf> {
    None // not confirmed yet (SPEC §11 backlog: install_dir 확정)
}

fn install(os: &OsInfo) -> Support<Vec<Command>> {
    match os.os {
        Os::MacOs | Os::Linux => Support::Supported(vec![Command::sh(&format!(
            "curl -fsSL {INSTALL_URL} | bash"
        ))]),
        Os::Windows if !os.is_windows_11() => {
            Support::Unsupported("Kiro requires Windows 11 (SPEC §7.4)")
        }
        Os::Windows => Support::Unsupported(
            "Kiro Windows install command not confirmed yet (SPEC §11 backlog)",
        ),
    }
}

fn update(_os: &OsInfo, source: InstallSource) -> Support<Vec<Command>> {
    match source {
        InstallSource::Native => Support::Supported(vec![Command::new("kiro-cli", &["update"])]),
        _ => Support::Unsupported("Kiro has no package-manager channel (SPEC §7.4)"),
    }
}
