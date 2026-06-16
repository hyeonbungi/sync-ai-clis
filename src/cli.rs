//! CLI surface (SPEC §6.1). Flags win over config (§6.2); usage errors exit
//! with code 2 (clap's default, matching §6.3).

use clap::{Parser, Subcommand};

use crate::config::{Config, InstallMissing};
use crate::engine::InstallPolicy;
use crate::tools::ToolSpec;

#[derive(Debug, Parser)]
#[command(
    name = "sync-ai-clis",
    version,
    about = "Detect, install, and keep AI coding CLIs up to date",
    long_about = "Reconciles the machine toward \"every known AI CLI installed, working, \
                  and current\": installed tools get updated through the channel they were \
                  installed with, missing tools are installed after consent, and every tool \
                  is re-verified (--version must actually run)."
)]
pub struct Cli {
    /// Non-interactive: install missing tools and update everything (CI-friendly)
    #[arg(short = 'y', long, conflicts_with = "no_install")]
    pub yes: bool,

    /// Update only; never offer to install missing tools
    #[arg(long)]
    pub no_install: bool,

    /// Only manage these tools (comma-separated ids, e.g. claude,gemini)
    #[arg(long, global = true, value_delimiter = ',', value_name = "IDS")]
    pub only: Vec<String>,

    /// Manage all tools except these (comma-separated ids)
    #[arg(long, global = true, value_delimiter = ',', value_name = "IDS")]
    pub except: Vec<String>,

    /// Print the exact commands that would run, execute nothing
    #[arg(long)]
    pub dry_run: bool,

    /// Emit the summary as JSON (automation-friendly)
    #[arg(long, global = true)] // also legal after subcommands: `doctor --json`
    pub json: bool,

    #[command(subcommand)]
    pub command: Option<Subcmd>,
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum Subcmd {
    /// Show known tools with installed state and current version
    #[command(alias = "status")]
    List,
    /// Diagnose broken, duplicate, or shadowed installs (read-only)
    Doctor,
    /// Check for available updates without changing anything (read-only)
    Check,
}

/// Flags win over config; config pins the non-interactive default
/// (SPEC §6.2). Default is Prompt.
pub fn install_policy(yes: bool, no_install: bool, config: &Config) -> InstallPolicy {
    if yes {
        return InstallPolicy::Always;
    }
    if no_install {
        return InstallPolicy::Never;
    }
    match config.install_missing {
        Some(InstallMissing::Always) => InstallPolicy::Always,
        Some(InstallMissing::Never) => InstallPolicy::Never,
        Some(InstallMissing::Prompt) | None => InstallPolicy::Prompt,
    }
}

/// Applies --only/--except (and the config `tools` allowlist) to the
/// registry, preserving registry order. Unknown ids are usage errors.
pub fn select_tools(
    registry: Vec<ToolSpec>,
    only: &[String],
    except: &[String],
    config_tools: Option<&[String]>,
) -> Result<Vec<ToolSpec>, String> {
    let known: Vec<&str> = registry.iter().map(|t| t.id).collect();
    let validate = |ids: &[String]| -> Result<(), String> {
        match ids.iter().find(|id| !known.contains(&id.as_str())) {
            Some(unknown) => Err(format!(
                "unknown tool id `{unknown}` (known ids: {})",
                known.join(", ")
            )),
            None => Ok(()),
        }
    };
    validate(only)?;
    validate(except)?;
    if let Some(config_ids) = config_tools {
        validate(config_ids)?;
    }

    Ok(registry
        .into_iter()
        .filter(|t| only.is_empty() || only.iter().any(|id| id == t.id))
        .filter(|t| config_tools.is_none_or(|ids| ids.iter().any(|id| id == t.id)))
        .filter(|t| !except.iter().any(|id| id == t.id))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(std::iter::once("sync-ai-clis").chain(args.iter().copied())).unwrap()
    }

    #[test]
    fn parses_default_invocation() {
        let cli = parse(&[]);
        assert!(!cli.yes && !cli.no_install && !cli.dry_run && !cli.json);
        assert!(cli.only.is_empty() && cli.except.is_empty());
        assert_eq!(cli.command, None);
    }

    #[test]
    fn parses_comma_separated_tool_lists() {
        let cli = parse(&["--only", "claude,gemini", "--except", "kiro"]);
        assert_eq!(cli.only, vec!["claude", "gemini"]);
        assert_eq!(cli.except, vec!["kiro"]);
    }

    #[test]
    fn yes_conflicts_with_no_install() {
        let err = Cli::try_parse_from(["sync-ai-clis", "--yes", "--no-install"]).unwrap_err();
        assert_eq!(err.exit_code(), 2); // usage error per SPEC §6.3
    }

    #[test]
    fn list_subcommand_has_status_alias() {
        assert_eq!(parse(&["list"]).command, Some(Subcmd::List));
        assert_eq!(parse(&["status"]).command, Some(Subcmd::List));
    }

    #[test]
    fn doctor_subcommand_parses() {
        assert_eq!(parse(&["doctor"]).command, Some(Subcmd::Doctor));
    }

    #[test]
    fn check_subcommand_parses() {
        assert_eq!(parse(&["check"]).command, Some(Subcmd::Check));
    }

    #[test]
    fn tool_filters_parse_after_doctor_subcommand() {
        let cli = parse(&["doctor", "--only", "gemini", "--except", "claude"]);
        assert_eq!(cli.command, Some(Subcmd::Doctor));
        assert_eq!(cli.only, vec!["gemini"]);
        assert_eq!(cli.except, vec!["claude"]);
    }

    #[test]
    fn flags_override_config_for_install_policy() {
        let prompt_config = Config::default();
        let always_config = Config {
            install_missing: Some(InstallMissing::Always),
            ..Config::default()
        };
        let never_config = Config {
            install_missing: Some(InstallMissing::Never),
            ..Config::default()
        };

        assert_eq!(
            install_policy(false, false, &prompt_config),
            InstallPolicy::Prompt
        );
        assert_eq!(
            install_policy(false, false, &always_config),
            InstallPolicy::Always
        );
        assert_eq!(
            install_policy(false, false, &never_config),
            InstallPolicy::Never
        );
        // Flags beat config in both directions.
        assert_eq!(
            install_policy(true, false, &never_config),
            InstallPolicy::Always
        );
        assert_eq!(
            install_policy(false, true, &always_config),
            InstallPolicy::Never
        );
    }

    #[test]
    fn selects_all_tools_by_default_in_registry_order() {
        let selected = select_tools(crate::tools::registry(), &[], &[], None).unwrap();
        let ids: Vec<&str> = selected.iter().map(|t| t.id).collect();
        assert_eq!(
            ids,
            vec!["claude", "codex", "gemini", "kiro", "antigravity"]
        );
    }

    #[test]
    fn only_filters_and_preserves_registry_order() {
        let only = vec!["gemini".to_string(), "claude".to_string()];
        let selected = select_tools(crate::tools::registry(), &only, &[], None).unwrap();
        let ids: Vec<&str> = selected.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec!["claude", "gemini"]); // registry order, not flag order
    }

    #[test]
    fn except_removes_tools() {
        let except = vec!["kiro".to_string(), "codex".to_string()];
        let selected = select_tools(crate::tools::registry(), &[], &except, None).unwrap();
        let ids: Vec<&str> = selected.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec!["claude", "gemini", "antigravity"]);
    }

    #[test]
    fn config_tools_act_as_allowlist_under_flags() {
        let config_tools = vec!["claude".to_string(), "kiro".to_string()];
        let except = vec!["kiro".to_string()];
        let selected =
            select_tools(crate::tools::registry(), &[], &except, Some(&config_tools)).unwrap();
        let ids: Vec<&str> = selected.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec!["claude"]);
    }

    #[test]
    fn unknown_tool_ids_are_usage_errors() {
        let only = vec!["clade".to_string()];
        let err = select_tools(crate::tools::registry(), &only, &[], None).unwrap_err();
        assert!(err.contains("clade"), "err was: {err}");
        assert!(err.contains("claude"), "should list known ids: {err}");
    }
}
