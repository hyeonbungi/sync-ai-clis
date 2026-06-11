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
/// Order matters: npm globals installed through a brew-managed Node live
/// under the Homebrew prefix but resolve into `node_modules`, so the npm
/// marker wins over brew markers.
pub fn classify_path(resolved_path: &str) -> InstallSource {
    let normalized = resolved_path.to_ascii_lowercase().replace('\\', "/");
    if normalized.contains("/node_modules/") {
        InstallSource::Npm
    } else if normalized.contains("/scoop/") {
        InstallSource::Scoop
    } else if normalized.contains("/winget/") {
        InstallSource::Winget
    } else if normalized.contains("/cellar/")
        || normalized.contains("/caskroom/")
        || normalized.contains("homebrew")
        || normalized.contains("linuxbrew")
    {
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
