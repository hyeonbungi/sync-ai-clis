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
use std::io::Read;
use std::path::{Path, PathBuf};

/// Finds a native executable or Windows command shim on the process PATH.
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
    find_all_in_path_env_for(bin, path_var, cfg!(windows))
}

pub fn find_all_in_path_env_for(bin: &str, path_var: &OsStr, windows: bool) -> Vec<PathBuf> {
    let mut matches = Vec::new();
    for dir in std::env::split_paths(path_var) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        if let Some(hit) = binary_candidates(&dir, bin, windows)
            .into_iter()
            .find(|candidate| candidate.is_file())
        {
            matches.push(hit);
        }
    }
    matches
}

/// Shared by PATH detection, post-install verification and doctor.
/// On Windows, never prefer npm's extensionless POSIX shell shim.
pub fn binary_candidates(dir: &Path, bin: &str, windows: bool) -> Vec<PathBuf> {
    if Path::new(bin).extension().is_some() {
        return vec![dir.join(bin)];
    }
    if windows {
        ["exe", "cmd", "bat"]
            .iter()
            .map(|ext| dir.join(format!("{bin}.{ext}")))
            .collect()
    } else {
        vec![dir.join(bin)]
    }
}

/// Resolve the extension before passing a Windows program to Rust's
/// process API; .cmd/.bat argument escaping remains Rust's responsibility.
#[cfg(windows)]
pub fn resolve_windows_program(program: &str) -> PathBuf {
    let path = Path::new(program);
    if path.components().count() > 1 || path.is_absolute() {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        binary_candidates(parent, &name, true)
            .into_iter()
            .find(|p| p.is_file())
            .unwrap_or_else(|| path.to_path_buf())
    } else {
        find_in_path(program).unwrap_or_else(|| path.to_path_buf())
    }
}

/// npm's Windows shims are ordinary files, not symlinks. Read only the
/// known quoted, shim-relative node_modules target forms; never execute a
/// shim to determine its owner or interpret arbitrary shell expressions.
fn installed_target(path: &Path) -> PathBuf {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if classify_path(&resolved.to_string_lossy()) != InstallSource::Native {
        return resolved;
    }
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !matches!(extension.to_ascii_lowercase().as_str(), "" | "cmd" | "ps1") {
        return resolved;
    }
    let Ok(file) = std::fs::File::open(&resolved) else {
        return resolved;
    };
    let mut text = String::new();
    if file.take(64 * 1024).read_to_string(&mut text).is_err() {
        return resolved;
    }
    let Some(parent) = resolved.parent() else {
        return resolved;
    };
    for marker in [
        "\"%dp0%\\",
        "\"%~dp0\\",
        "\"$basedir/",
        "\"$basedir\\",
        "\"$basedir_win/",
    ] {
        for tail in text.split(marker).skip(1) {
            let Some(relative) = tail.split('"').next() else {
                continue;
            };
            let relative = relative.replace('\\', "/");
            if !relative.starts_with("node_modules/")
                || relative.split('/').any(|part| matches!(part, ".." | "."))
            {
                continue;
            }
            let target = parent.join(relative);
            if parent.join("node_modules").is_dir() {
                return std::fs::canonicalize(&target).unwrap_or(target);
            }
        }
    }
    resolved
}

/// Classifies how a binary was installed from its **resolved** path.
/// Order matters twice over: Cellar/Caskroom store paths are definitive
/// (brew formulas may vendor npm packages inside the Cellar — footool
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
    let resolved = installed_target(path);
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
    let resolved = installed_target(bin_path);
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
            classify_path("/opt/homebrew/Cellar/footool/0.9.0/bin/footool"),
            InstallSource::Brew
        );
        assert_eq!(
            classify_path("/opt/homebrew/Caskroom/codex/1.0.0/codex"),
            InstallSource::Brew
        );
        assert_eq!(
            classify_path("/home/linuxbrew/.linuxbrew/bin/footool"),
            InstallSource::Brew
        );
    }

    #[test]
    fn npm_marker_wins_over_brew_prefix() {
        // npm -g via brew-managed Node resolves under the homebrew prefix.
        assert_eq!(
            classify_path("/opt/homebrew/lib/node_modules/@example/footool/bundle/footool.js"),
            InstallSource::Npm
        );
        assert_eq!(
            classify_path("/usr/local/lib/node_modules/@openai/codex/bin/codex.js"),
            InstallSource::Npm
        );
    }

    #[test]
    fn formula_and_cask_store_paths_beat_vendored_node_modules() {
        // footool's brew formula vendors the npm package inside its
        // Cellar — the store path is definitive, and updating through npm
        // would create a second copy instead (SPEC §11, found live).
        assert_eq!(
            classify_path(
                "/opt/homebrew/Cellar/footool/0.45.2/libexec/lib/node_modules/@example/footool/bundle/footool.js"
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
        std::fs::write(
            first.join(if cfg!(windows) {
                "footool.exe"
            } else {
                "footool"
            }),
            "#!/bin/sh\n",
        )
        .unwrap();
        std::fs::write(
            second.join(if cfg!(windows) {
                "footool.exe"
            } else {
                "footool"
            }),
            "#!/bin/sh\n",
        )
        .unwrap();

        let path_var = std::env::join_paths([first.clone(), second.clone()]).unwrap();
        assert_eq!(
            find_all_in_path_env("footool", &path_var),
            vec![
                first.join(if cfg!(windows) {
                    "footool.exe"
                } else {
                    "footool"
                }),
                second.join(if cfg!(windows) {
                    "footool.exe"
                } else {
                    "footool"
                })
            ],
            "every match, PATH order"
        );
        assert!(find_all_in_path_env("missing-tool", &path_var).is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn finds_binaries_on_an_injected_path() {
        let dir = std::env::temp_dir().join(format!("sync-ai-clis-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join(if cfg!(windows) {
            "footool.exe"
        } else {
            "footool"
        });
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
        let store = root.join("lib").join("node_modules").join("@example");
        std::fs::create_dir_all(&store).unwrap();
        let target = store.join("footool.js");
        std::fs::write(&target, "// stub").unwrap();
        let link = root.join("footool");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert_eq!(detect_from_path(&link), InstallSource::Npm);
        // A plain file with no markers stays Native.
        let plain = root.join("agy");
        std::fs::write(&plain, "stub").unwrap();
        assert_eq!(detect_from_path(&plain), InstallSource::Native);

        std::fs::remove_dir_all(&root).ok();
    }
}

#[cfg(test)]
mod windows_regression_tests {
    use super::*;

    #[test]
    fn windows_lookup_ignores_posix_shim_and_selects_native_commands() {
        let dir = std::env::temp_dir().join(format!("sync-lookup-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = std::env::join_paths([&dir]).unwrap();
        std::fs::write(dir.join("footool"), "#!/bin/sh\n").unwrap();
        std::fs::write(dir.join("footool.ps1"), "# powershell shim\n").unwrap();
        assert!(find_all_in_path_env_for("footool", &path, true).is_empty());
        std::fs::write(dir.join("footool.cmd"), "@echo off\n").unwrap();
        assert_eq!(
            find_all_in_path_env_for("footool", &path, true),
            vec![dir.join("footool.cmd")]
        );
        std::fs::write(dir.join("footool.exe"), "fixture").unwrap();
        assert_eq!(
            find_all_in_path_env_for("footool", &path, true),
            vec![dir.join("footool.exe")]
        );
        assert_eq!(
            find_all_in_path_env_for("footool.cmd", &path, true),
            vec![dir.join("footool.cmd")]
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn npm_shims_resolve_source_and_owner_without_executing() {
        let root = std::env::temp_dir().join(format!("sync-shim-{}", std::process::id()));
        let package = root.join("node_modules/@example/footool");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(root.join("npm.cmd"), "@echo off\r\n").unwrap();
        // Missing JS entry points must still be classified as npm so that
        // a broken package can be repaired through its original channel.
        let root = std::fs::canonicalize(&root).unwrap();
        let target = root.join("node_modules/@example/footool/missing.js");
        for (index, content) in [
            r#"@echo off
node "%dp0%\node_modules\@example\footool\missing.js" %*"#,
            r#"& "$basedir/node_modules/@example/footool/missing.js" $args"#,
            r#"exec node "$basedir_win/node_modules/@example/footool/missing.js" "$@""#,
        ]
        .iter()
        .enumerate()
        {
            let shim = root.join(format!("footool-{index}.cmd"));
            std::fs::write(&shim, content).unwrap();
            assert_eq!(installed_target(&shim), target);
            assert_eq!(detect_from_path(&shim), InstallSource::Npm);
            assert_eq!(owning_npm(&shim), Some(root.join("npm.cmd")));
        }
        let unrelated = root.join("unrelated.cmd");
        std::fs::write(
            &unrelated,
            r#"echo node_modules; node "%dp0%\node_modules\..\outside.js""#,
        )
        .unwrap();
        assert_eq!(detect_from_path(&unrelated), InstallSource::Native);
        assert_eq!(owning_npm(&unrelated), None);
        std::fs::remove_dir_all(root).unwrap();
    }
}
