//! Declarative tool registry (SPEC §5.2). Adding a tool = one module with a
//! `ToolSpec` + one line in `registry()`. Install/update URLs must be
//! hardcoded official HTTPS constants — never user- or config-supplied
//! (SPEC §5.5).

use std::path::PathBuf;

use crate::os::OsInfo;
use crate::runner::Command;
use crate::source::InstallSource;

/// Supported with a payload, or unsupported with a human-readable reason
/// that the report shows as a SKIP (SPEC §5.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Support<T> {
    Supported(T),
    Unsupported(&'static str),
}

impl<T> Support<T> {
    pub fn supported(self) -> Option<T> {
        match self {
            Support::Supported(value) => Some(value),
            Support::Unsupported(_) => None,
        }
    }

    pub fn unsupported_reason(&self) -> Option<&'static str> {
        match self {
            Support::Supported(_) => None,
            Support::Unsupported(reason) => Some(reason),
        }
    }
}

/// How `check` learns a tool's latest available version (read-only, design
/// doc 0012). `Probe` runs a command and extracts the version from its
/// stdout; `SelfUpdating` marks tools that update themselves in the
/// background, so a "behind" check is not meaningful.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LatestSource {
    Probe { command: Command, extract: Extract },
    SelfUpdating,
}

/// How to pull a version string out of a probe's stdout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extract {
    /// stdout is the version itself (`npm view <pkg> version`).
    Raw,
    /// stdout is JSON; take this top-level string key (agy manifest).
    JsonKey(&'static str),
}

/// One managed AI CLI, described as data plus per-OS/per-source plan
/// functions (SPEC §5.2 — function pointers keep OS/source branching
/// expressible while tool addition stays declarative).
#[derive(Debug)]
pub struct ToolSpec {
    pub id: &'static str,
    pub display: &'static str,
    /// Binary looked up on PATH and re-verified with `version_args`.
    pub bin: &'static str,
    pub version_args: &'static [&'static str],
    /// Known absolute install dir for the PATH-not-yet-refreshed re-check
    /// after a fresh install (SPEC §5.5). None = no fixed path is known.
    pub install_dir: fn(&OsInfo) -> Option<PathBuf>,
    /// True when the tool background-self-updates (Claude, Kiro).
    pub self_updates: bool,
    pub install: fn(&OsInfo) -> Support<Vec<Command>>,
    pub update: fn(&OsInfo, InstallSource) -> Support<Vec<Command>>,
    /// Recovery when the binary exists but does not work (Codex reinstall,
    /// inherited from the original script).
    pub on_broken: Option<fn(&OsInfo, InstallSource) -> Vec<Command>>,
    /// How `check` learns the latest available version (design doc 0012).
    pub latest_source: fn(&OsInfo) -> LatestSource,
    /// Current-OS install script URL for `audit` change detection (design
    /// doc 0013). Always a §5.5 hardcoded official HTTPS const; None when the
    /// tool has no remote install script (gemini installs via npm).
    pub install_script: fn(&OsInfo) -> Option<&'static str>,
}

mod antigravity;
mod claude;
mod codex;
mod gemini;
mod kiro;

/// All known tools, in display order.
pub fn registry() -> Vec<ToolSpec> {
    vec![
        claude::spec(),
        codex::spec(),
        gemini::spec(),
        kiro::spec(),
        antigravity::spec(),
    ]
}
