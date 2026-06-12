//! Install-source classification (SPEC §5.1). A tool already installed via a
//! package manager is updated through that same channel instead of being
//! installed twice (SPEC §4 insight). Detection logic lands with P1-004;
//! the enum lives here because ToolSpec update plans key on it.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallSource {
    /// Official native installer / self-managed binary (the cross-platform default).
    Native,
    /// Homebrew formula or cask.
    Brew,
    /// npm global package.
    Npm,
    /// Windows winget.
    Winget,
    /// Windows Scoop.
    Scoop,
}

impl InstallSource {
    pub fn from_channel_name(name: &str) -> Option<InstallSource> {
        match name {
            "native" => Some(InstallSource::Native),
            "brew" => Some(InstallSource::Brew),
            "npm" => Some(InstallSource::Npm),
            "winget" => Some(InstallSource::Winget),
            "scoop" => Some(InstallSource::Scoop),
            _ => None,
        }
    }

    pub fn channel_name(self) -> &'static str {
        match self {
            InstallSource::Native => "native",
            InstallSource::Brew => "brew",
            InstallSource::Npm => "npm",
            InstallSource::Winget => "winget",
            InstallSource::Scoop => "scoop",
        }
    }
}

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Finds `bin` on the process PATH (`bin.exe` is also tried, for Windows).
pub fn find_in_path(bin: &str) -> Option<PathBuf> {
    find_in_path_env(bin, std::env::var_os("PATH")?.as_ref())
}

/// PATH-injectable lookup so the search order is testable without mutating
/// process env (env mutation is unsafe in edition 2024).
pub fn find_in_path_env(bin: &str, path_var: &OsStr) -> Option<PathBuf> {
    find_all_in_path_env(bin, path_var).into_iter().next()
}

/// Every PATH match in search order (one per directory) — `doctor` needs the
/// shadowed copies that `find_in_path_env` skips past.
pub fn find_all_in_path_env(bin: &str, path_var: &OsStr) -> Vec<PathBuf> {
    let mut matches = Vec::new();
    for dir in std::env::split_paths(path_var) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        if let Some(hit) = [dir.join(bin), dir.join(format!("{bin}.exe"))]
            .into_iter()
            .find(|candidate| candidate.is_file())
        {
            matches.push(hit);
        }
    }
    matches
}

/// Classifies how a binary was installed from its **resolved** path.
/// Order matters twice over: Cellar/Caskroom store paths are definitive
/// (brew formulas may vendor npm packages inside the Cellar — gemini-cli
/// does — and updating those through npm creates a duplicate, SPEC §11);
/// below the store level, npm globals installed through a brew-managed
/// Node live under the bare Homebrew prefix but resolve into
/// `node_modules`, so there the npm marker wins over the prefix markers.
pub fn classify_path(resolved_path: &str) -> InstallSource {
    let normalized = resolved_path.to_ascii_lowercase().replace('\\', "/");
    if normalized.contains("/cellar/") || normalized.contains("/caskroom/") {
        InstallSource::Brew
    } else if normalized.contains("/node_modules/") {
        InstallSource::Npm
    } else if normalized.contains("/scoop/") {
        InstallSource::Scoop
    } else if normalized.contains("/winget/") {
        InstallSource::Winget
    } else if normalized.contains("homebrew") || normalized.contains("linuxbrew") {
        InstallSource::Brew
    } else {
        InstallSource::Native
    }
}

/// Resolves symlinks (npm/brew shims point into their stores) and classifies.
/// Unresolvable paths classify as found.
pub fn detect_from_path(path: &Path) -> InstallSource {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    classify_path(&resolved.to_string_lossy())
}

/// The npm executable that owns an npm-global install, resolved from the
/// installed binary's real location: `<prefix>/lib/node_modules/…` pairs
/// with `<prefix>/bin/npm` (unix), `<prefix>/node_modules/…` with
/// `<prefix>/npm.cmd` (windows). `npm install -g` through any *other* npm
/// lands in that npm's own prefix (nvm setups) and creates a duplicate
/// instead of an update (SPEC §11, found live 2026-06-11). None when the
/// layout doesn't match — callers fall back to plain `npm`.
pub fn owning_npm(bin_path: &Path) -> Option<PathBuf> {
    let resolved = std::fs::canonicalize(bin_path).unwrap_or_else(|_| bin_path.to_path_buf());
    for ancestor in resolved.ancestors() {
        if ancestor
            .file_name()
            .is_none_or(|name| name != "node_modules")
        {
            continue;
        }
        let Some(holder) = ancestor.parent() else {
            continue;
        };
        // unix layout: <prefix>/lib/node_modules → <prefix>/bin/npm
        if holder.file_name().is_some_and(|name| name == "lib")
            && let Some(prefix) = holder.parent()
        {
            let npm = prefix.join("bin").join("npm");
            if npm.is_file() {
                return Some(npm);
            }
        }
        // windows layout: <prefix>/node_modules → npm.cmd beside it
        for name in ["npm.cmd", "npm"] {
            let npm = holder.join(name);
            if npm.is_file() {
                return Some(npm);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_brew_cellar_and_caskroom_and_linuxbrew() {
        assert_eq!(
            classify_path("/opt/homebrew/Cellar/gemini-cli/0.9.0/bin/gemini"),
            InstallSource::Brew
        );
        assert_eq!(
            classify_path("/opt/homebrew/Caskroom/codex/1.0.0/codex"),
            InstallSource::Brew
        );
        assert_eq!(
            classify_path("/home/linuxbrew/.linuxbrew/bin/gemini"),
            InstallSource::Brew
        );
    }

    #[test]
    fn npm_marker_wins_over_brew_prefix() {
        // npm -g via brew-managed Node resolves under the homebrew prefix.
        assert_eq!(
            classify_path("/opt/homebrew/lib/node_modules/@google/gemini-cli/bundle/gemini.js"),
            InstallSource::Npm
        );
        assert_eq!(
            classify_path("/usr/local/lib/node_modules/@openai/codex/bin/codex.js"),
            InstallSource::Npm
        );
    }

    #[test]
    fn formula_and_cask_store_paths_beat_vendored_node_modules() {
        // gemini-cli's brew formula vendors the npm package inside its
        // Cellar — the store path is definitive, and updating through npm
        // would create a second copy instead (SPEC §11, found live).
        assert_eq!(
            classify_path(
                "/opt/homebrew/Cellar/gemini-cli/0.45.2/libexec/lib/node_modules/@google/gemini-cli/bundle/gemini.js"
            ),
            InstallSource::Brew
        );
        assert_eq!(
            classify_path("/opt/homebrew/Caskroom/sometool/1.0/payload/node_modules/cli.js"),
            InstallSource::Brew
        );
    }

    #[cfg(unix)]
    #[test]
    fn owning_npm_resolves_the_prefix_that_holds_the_install() {
        // unix npm layout: <prefix>/lib/node_modules/<pkg> ↔ <prefix>/bin/npm
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("sync-own-npm-{}", std::process::id()));
        let store = root.join("prefix/lib/node_modules/footool/bin");
        std::fs::create_dir_all(&store).unwrap();
        let target = store.join("footool.js");
        std::fs::write(&target, "// stub").unwrap();
        let bin_dir = root.join("prefix/bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(bin_dir.join("npm"), "#!/bin/sh\n").unwrap();
        let shim = bin_dir.join("footool");
        std::os::unix::fs::symlink(&target, &shim).unwrap();

        assert_eq!(owning_npm(&shim), Some(bin_dir.join("npm")));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn owning_npm_handles_the_windows_flat_layout() {
        // windows npm layout: <prefix>/node_modules/<pkg> with npm.cmd
        // sitting directly in <prefix>.
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("sync-own-npm-win-{}", std::process::id()));
        let store = root.join("npmprefix/node_modules/footool");
        std::fs::create_dir_all(&store).unwrap();
        let target = store.join("footool.js");
        std::fs::write(&target, "// stub").unwrap();
        std::fs::write(root.join("npmprefix/npm.cmd"), "stub").unwrap();

        assert_eq!(owning_npm(&target), Some(root.join("npmprefix/npm.cmd")));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn owning_npm_is_none_outside_node_modules() {
        let root = std::env::temp_dir().join(format!("sync-own-none-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let plain = root.join("agy");
        std::fs::write(&plain, "stub").unwrap();

        assert_eq!(owning_npm(&plain), None);

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn classifies_windows_scoop_and_winget() {
        assert_eq!(
            classify_path(r"C:\Users\dev\scoop\shims\claude.exe"),
            InstallSource::Scoop
        );
        assert_eq!(
            classify_path(r"C:\Users\dev\AppData\Local\Microsoft\WinGet\Links\claude.exe"),
            InstallSource::Winget
        );
    }

    #[test]
    fn unrecognized_paths_are_native() {
        assert_eq!(
            classify_path("/Users/dev/.local/bin/agy"),
            InstallSource::Native
        );
        assert_eq!(
            classify_path("/usr/local/bin/claude"),
            InstallSource::Native
        );
        assert_eq!(classify_path(""), InstallSource::Native);
    }

    #[test]
    fn finds_every_path_match_in_order() {
        let root = std::env::temp_dir().join(format!("sync-ai-clis-all-{}", std::process::id()));
        let (first, second) = (root.join("a"), root.join("b"));
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        std::fs::write(first.join("footool"), "#!/bin/sh\n").unwrap();
        std::fs::write(second.join("footool"), "#!/bin/sh\n").unwrap();

        let path_var = std::env::join_paths([first.clone(), second.clone()]).unwrap();
        assert_eq!(
            find_all_in_path_env("footool", &path_var),
            vec![first.join("footool"), second.join("footool")],
            "every match, PATH order"
        );
        assert!(find_all_in_path_env("missing-tool", &path_var).is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn finds_binaries_on_an_injected_path() {
        let dir = std::env::temp_dir().join(format!("sync-ai-clis-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("footool");
        std::fs::write(&bin, "#!/bin/sh\n").unwrap();

        let path_var = std::env::join_paths([dir.clone()]).unwrap();
        assert_eq!(find_in_path_env("footool", &path_var), Some(bin.clone()));
        assert_eq!(find_in_path_env("missing-tool", &path_var), None);

        std::fs::remove_file(&bin).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn detect_resolves_symlinks_before_classifying() {
        let root = std::env::temp_dir().join(format!("sync-ai-clis-link-{}", std::process::id()));
        let store = root.join("lib").join("node_modules").join("@google");
        std::fs::create_dir_all(&store).unwrap();
        let target = store.join("gemini.js");
        std::fs::write(&target, "// stub").unwrap();
        let link = root.join("gemini");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert_eq!(detect_from_path(&link), InstallSource::Npm);
        // A plain file with no markers stays Native.
        let plain = root.join("agy");
        std::fs::write(&plain, "stub").unwrap();
        assert_eq!(detect_from_path(&plain), InstallSource::Native);

        std::fs::remove_dir_all(&root).ok();
    }
}
