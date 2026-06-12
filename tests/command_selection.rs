//! OS × install-state × source command-selection matrix (SPEC §8.1).
//! Exercises the declarative registry from any host without touching the
//! system: plans are data, never executed here.

use sync_ai_clis::os::{Libc, Os, OsInfo};
use sync_ai_clis::source::InstallSource;
use sync_ai_clis::tools::{ToolSpec, registry};

fn macos() -> OsInfo {
    OsInfo {
        os: Os::MacOs,
        arch: "aarch64".into(),
        windows_build: None,
        libc: None,
    }
}

fn windows(build: u32) -> OsInfo {
    OsInfo {
        os: Os::Windows,
        arch: "x86_64".into(),
        windows_build: Some(build),
        libc: None,
    }
}

fn linux_glibc(major: u32, minor: u32) -> OsInfo {
    OsInfo {
        os: Os::Linux,
        arch: "x86_64".into(),
        windows_build: None,
        libc: Some(Libc::Glibc { major, minor }),
    }
}

fn spec(id: &str) -> ToolSpec {
    registry()
        .into_iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("registry has no tool {id}"))
}

fn rendered(plan: sync_ai_clis::tools::Support<Vec<sync_ai_clis::runner::Command>>) -> Vec<String> {
    plan.supported()
        .expect("expected a supported plan")
        .iter()
        .map(|c| c.to_string())
        .collect()
}

#[test]
fn registry_lists_five_tools_with_unique_ids() {
    let specs = registry();
    let ids: Vec<&str> = specs.iter().map(|s| s.id).collect();
    assert_eq!(
        ids,
        vec!["claude", "codex", "gemini", "kiro", "antigravity"]
    );
    let bins: Vec<&str> = specs.iter().map(|s| s.bin).collect();
    assert_eq!(bins, vec!["claude", "codex", "gemini", "kiro-cli", "agy"]);
    assert!(specs.iter().all(|s| s.version_args == ["--version"]));
}

#[test]
fn claude_installs_via_official_installer_per_os() {
    let tool = spec("claude");
    assert_eq!(
        rendered((tool.install)(&macos())),
        vec![r#"sh -c "curl -fsSL https://claude.ai/install.sh | bash""#]
    );
    assert_eq!(
        rendered((tool.install)(&windows(22631))),
        vec![
            r#"powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://claude.ai/install.ps1 | iex""#
        ]
    );
}

#[test]
fn claude_updates_through_the_detected_source() {
    let tool = spec("claude");
    let on = |source| rendered((tool.update)(&macos(), source));
    assert_eq!(on(InstallSource::Native), vec!["claude update"]);
    assert_eq!(on(InstallSource::Brew), vec!["brew upgrade claude-code"]);
    assert_eq!(
        on(InstallSource::Npm),
        vec!["npm install -g @anthropic-ai/claude-code@latest"]
    );
    assert_eq!(
        rendered((tool.update)(&windows(22631), InstallSource::Winget)),
        vec!["winget upgrade Anthropic.ClaudeCode"]
    );
    // SPEC §7.1: scoop package not confirmed — explicit skip, not a guess.
    assert!(
        (tool.update)(&windows(22631), InstallSource::Scoop)
            .unsupported_reason()
            .is_some()
    );
    assert!(tool.self_updates);
}

#[test]
fn codex_missing_on_linux_installs_via_native_installer() {
    // SPEC §8.1 example: (os=linux, tool=codex, 미설치, --yes) → native installer.
    assert_eq!(
        rendered((spec("codex").install)(&linux_glibc(2, 39))),
        vec![r#"sh -c "curl -fsSL https://chatgpt.com/codex/install.sh | sh""#]
    );
}

#[test]
fn codex_updates_by_source_and_uses_native_self_update() {
    let tool = spec("codex");
    assert_eq!(
        rendered((tool.update)(&macos(), InstallSource::Brew)),
        vec!["brew upgrade --cask codex"]
    );
    // Codex brew cask is mac-only (SPEC §7.2).
    assert!(
        (tool.update)(&linux_glibc(2, 39), InstallSource::Brew)
            .unsupported_reason()
            .is_some()
    );
    assert_eq!(
        rendered((tool.update)(&macos(), InstallSource::Npm)),
        vec!["npm install -g @openai/codex@latest"]
    );
    // Native update = Codex's official self-update command.
    assert_eq!(
        rendered((tool.update)(&macos(), InstallSource::Native)),
        vec!["codex update"]
    );
}

#[test]
fn codex_on_broken_reinstalls_per_source() {
    let tool = spec("codex");
    let on_broken = tool.on_broken.expect("codex keeps the recovery hook");
    let render = |cmds: Vec<sync_ai_clis::runner::Command>| {
        cmds.iter().map(|c| c.to_string()).collect::<Vec<_>>()
    };
    assert_eq!(
        render(on_broken(&macos(), InstallSource::Brew)),
        vec!["brew reinstall --cask codex"]
    );
    assert_eq!(
        render(on_broken(&linux_glibc(2, 39), InstallSource::Native)),
        vec![r#"sh -c "curl -fsSL https://chatgpt.com/codex/install.sh | sh""#]
    );
}

#[test]
fn gemini_prefers_brew_when_installed_that_way() {
    // SPEC §8.1 example: (os=macos, tool=gemini, source=brew) → brew upgrade gemini-cli.
    assert_eq!(
        rendered((spec("gemini").update)(&macos(), InstallSource::Brew)),
        vec!["brew upgrade gemini-cli"]
    );
}

#[test]
fn gemini_installs_via_npm_and_has_no_native_channel() {
    let tool = spec("gemini");
    assert_eq!(
        rendered((tool.install)(&linux_glibc(2, 39))),
        vec!["npm install -g @google/gemini-cli"]
    );
    assert_eq!(
        rendered((tool.update)(&windows(22631), InstallSource::Npm)),
        vec!["npm install -g @google/gemini-cli@latest"]
    );
    assert!(
        (tool.update)(&macos(), InstallSource::Native)
            .unsupported_reason()
            .is_some()
    );
    // brew on Windows is not a thing.
    assert!(
        (tool.update)(&windows(22631), InstallSource::Brew)
            .unsupported_reason()
            .is_some()
    );
}

#[test]
fn gemini_has_no_fixed_install_dir() {
    // npm/brew globals vary by package-manager prefix, so a static
    // post-install recheck path would be a guess.
    assert!((spec("gemini").install_dir)(&macos()).is_none());
}

#[test]
fn kiro_skips_windows_10_with_a_clear_reason() {
    // SPEC §8.1 example: (os=windows10, tool=kiro) → SKIP("Win11 필요").
    let reason = (spec("kiro").install)(&windows(19045))
        .unsupported_reason()
        .expect("Win10 must be a skip");
    assert!(reason.contains("Windows 11"), "reason was: {reason}");
}

#[test]
fn kiro_installs_on_windows_11_with_official_powershell_installer() {
    assert_eq!(
        rendered((spec("kiro").install)(&windows(22631))),
        vec![
            r#"powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://cli.kiro.dev/install.ps1 | iex""#
        ]
    );
}

#[test]
fn kiro_installs_on_unix_and_self_updates() {
    let tool = spec("kiro");
    assert_eq!(
        rendered((tool.install)(&macos())),
        vec![r#"sh -c "curl -fsSL https://cli.kiro.dev/install | bash""#]
    );
    assert_eq!(
        rendered((tool.install)(&linux_glibc(2, 31))),
        vec![r#"sh -c "curl -fsSL https://cli.kiro.dev/install | bash""#]
    );
    assert_eq!(
        rendered((tool.update)(&linux_glibc(2, 39), InstallSource::Native)),
        vec!["kiro-cli update --non-interactive"]
    );
    assert!(
        (tool.update)(&macos(), InstallSource::Brew)
            .unsupported_reason()
            .is_some()
    );
    assert!(tool.self_updates);
}

#[test]
fn antigravity_uses_official_installer_and_agy_update() {
    let tool = spec("antigravity");
    assert_eq!(
        rendered((tool.install)(&linux_glibc(2, 39))),
        vec![r#"sh -c "curl -fsSL https://antigravity.google/cli/install.sh | bash""#]
    );
    assert_eq!(
        rendered((tool.install)(&windows(22631))),
        vec![
            r#"powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://antigravity.google/cli/install.ps1 | iex""#
        ]
    );
    assert_eq!(
        rendered((tool.update)(&macos(), InstallSource::Native)),
        vec!["agy update"]
    );
    assert!(
        (tool.update)(&macos(), InstallSource::Npm)
            .unsupported_reason()
            .is_some()
    );
}

#[test]
fn antigravity_knows_its_install_dir_for_path_recheck() {
    // SPEC §7.5: ~/.local/bin/agy on unix (confirmed); %LOCALAPPDATA%\agy\bin on Windows.
    let tool = spec("antigravity");
    let dir = (tool.install_dir)(&macos()).expect("unix install dir is known");
    assert!(dir.ends_with(".local/bin"), "dir was: {}", dir.display());
}

// ---- P1-008: remaining OS × source matrix coverage ----

#[test]
fn claude_linux_install_and_source_updates() {
    let tool = spec("claude");
    assert_eq!(
        rendered((tool.install)(&linux_glibc(2, 39))),
        vec![r#"sh -c "curl -fsSL https://claude.ai/install.sh | bash""#]
    );
    assert_eq!(
        rendered((tool.update)(&linux_glibc(2, 39), InstallSource::Npm)),
        vec!["npm install -g @anthropic-ai/claude-code@latest"]
    );
    // Native self-update works on every OS, including Windows.
    assert_eq!(
        rendered((tool.update)(&windows(22631), InstallSource::Native)),
        vec!["claude update"]
    );
}

#[test]
fn codex_windows_paths_use_wrapped_powershell() {
    let tool = spec("codex");
    let expected = r#"powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://chatgpt.com/codex/install.ps1 | iex""#;
    assert_eq!(rendered((tool.install)(&windows(22631))), vec![expected]);
    // Native update uses the self-update subcommand on Windows too.
    assert_eq!(
        rendered((tool.update)(&windows(22631), InstallSource::Native)),
        vec!["codex update"]
    );
    let on_broken = tool.on_broken.expect("codex recovery hook");
    let recovery: Vec<String> = on_broken(&windows(22631), InstallSource::Native)
        .iter()
        .map(|c| c.to_string())
        .collect();
    assert_eq!(recovery, vec![expected]);
}

#[test]
fn gemini_brew_update_also_works_on_linux() {
    assert_eq!(
        rendered((spec("gemini").update)(
            &linux_glibc(2, 39),
            InstallSource::Brew
        )),
        vec!["brew upgrade gemini-cli"]
    );
}

#[test]
fn kiro_installed_on_windows11_self_updates() {
    // Windows 11 install and update commands are both confirmed in SPEC §7.4.
    assert_eq!(
        rendered((spec("kiro").update)(
            &windows(22631),
            InstallSource::Native
        )),
        vec!["kiro-cli update --non-interactive"]
    );
}

#[test]
fn self_update_flags_match_spec() {
    // SPEC §5.2: background self-updaters are Claude and Kiro.
    let flags: Vec<(&str, bool)> = registry().iter().map(|t| (t.id, t.self_updates)).collect();
    assert_eq!(
        flags,
        vec![
            ("claude", true),
            ("codex", false),
            ("gemini", false),
            ("kiro", true),
            ("antigravity", false),
        ]
    );
}

#[test]
fn codex_knows_standalone_install_dirs_for_path_recheck() {
    let tool = spec("codex");
    let unix_dir = (tool.install_dir)(&macos()).expect("codex unix install dir is known");
    assert!(
        unix_dir.ends_with(".local/bin"),
        "dir was: {}",
        unix_dir.display()
    );

    let win_dir = (tool.install_dir)(&windows(22631)).expect("codex windows install dir is known");
    assert!(
        win_dir.ends_with("Programs/OpenAI/Codex/bin")
            || win_dir.ends_with(r"Programs\OpenAI\Codex\bin"),
        "dir was: {}",
        win_dir.display()
    );
}

#[test]
fn kiro_knows_confirmed_install_dirs_for_path_recheck() {
    let tool = spec("kiro");
    let mac_dir = (tool.install_dir)(&macos()).expect("kiro mac install dir is known");
    assert!(
        mac_dir.ends_with("Kiro CLI.app/Contents/MacOS")
            || mac_dir.ends_with(r"Kiro CLI.app\Contents\MacOS"),
        "dir was: {}",
        mac_dir.display()
    );

    let linux_dir =
        (tool.install_dir)(&linux_glibc(2, 39)).expect("kiro linux install dir is known");
    assert!(
        linux_dir.ends_with(".local/bin"),
        "dir was: {}",
        linux_dir.display()
    );

    let win_dir = (tool.install_dir)(&windows(22631)).expect("kiro windows install dir is known");
    assert!(
        win_dir.ends_with("Kiro-Cli") || win_dir.ends_with(r"Kiro-Cli"),
        "dir was: {}",
        win_dir.display()
    );
}

#[test]
fn every_remote_install_plan_uses_https_only() {
    // Security invariant (SPEC §5.5): remote installer commands must point
    // at hardcoded official HTTPS URLs; plain http is forbidden.
    let oses = [macos(), windows(22631), linux_glibc(2, 39)];
    for tool in registry() {
        for os in &oses {
            if let Some(cmds) = (tool.install)(os).supported() {
                for cmd in cmds {
                    let rendered = cmd.to_string();
                    if rendered.contains("curl") || rendered.contains("irm") {
                        assert!(
                            rendered.contains("https://"),
                            "{}: non-https installer: {rendered}",
                            tool.id
                        );
                    }
                    assert!(
                        !rendered.contains("http://"),
                        "{}: plain http forbidden: {rendered}",
                        tool.id
                    );
                }
            }
        }
    }
}

#[test]
fn claude_knows_its_install_dir_for_path_recheck() {
    // CI-confirmed (integration run 27285726406): the official installer
    // places claude at ~/.local/bin (unix) and %USERPROFILE%\.local\bin
    // (Windows) — the same home-relative dir on every OS.
    let tool = spec("claude");
    let dir = (tool.install_dir)(&macos()).expect("claude install dir is known");
    assert!(dir.ends_with(".local/bin"), "dir was: {}", dir.display());
}
