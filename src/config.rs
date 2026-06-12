//! `config.toml` loading (SPEC §6.2): `~/.config/sync-ai-clis/config.toml`
//! on macOS/Linux, the platform config dir (%APPDATA%) on Windows. Flags
//! always win over config.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

use crate::source::InstallSource;
use crate::tools::ToolSpec;

#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
pub struct Config {
    /// Tools to manage (default: every known tool).
    pub tools: Option<Vec<String>>,
    /// How to treat missing tools: prompt | always | never.
    pub install_missing: Option<InstallMissing>,
    /// Per-tool preferred channel overrides for update planning.
    pub channels: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallMissing {
    Prompt,
    Always,
    Never,
}

/// Parses config text (pure, testable).
pub fn parse(text: &str) -> Result<Config, toml::de::Error> {
    toml::from_str(text)
}

/// Platform config path per SPEC §6.2: literal `~/.config/...` on unix,
/// the platform config dir (%APPDATA%) on Windows.
pub fn config_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        dirs::config_dir().map(|dir| dir.join("sync-ai-clis").join("config.toml"))
    }
    #[cfg(not(windows))]
    {
        dirs::home_dir().map(|home| {
            home.join(".config")
                .join("sync-ai-clis")
                .join("config.toml")
        })
    }
}

/// Loads the config file; a missing file is the default config, a malformed
/// file is an error the caller should surface (exit 2).
pub fn load() -> Result<Config, String> {
    let Some(path) = config_path() else {
        return Ok(Config::default());
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => parse(&text).map_err(|e| format!("invalid config {}: {e}", path.display())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(err) => Err(format!("could not read {}: {err}", path.display())),
    }
}

pub fn channel_overrides(
    channels: Option<&HashMap<String, String>>,
    registry: &[ToolSpec],
) -> Result<HashMap<String, InstallSource>, String> {
    let Some(channels) = channels else {
        return Ok(HashMap::new());
    };
    let known: Vec<&str> = registry.iter().map(|t| t.id).collect();
    let known_channels = ["native", "brew", "npm", "winget", "scoop"];
    let mut mapped = HashMap::new();
    for (tool_id, channel) in channels {
        if !known.contains(&tool_id.as_str()) {
            return Err(format!(
                "unknown channel override tool id `{tool_id}` (known ids: {})",
                known.join(", ")
            ));
        }
        let Some(source) = InstallSource::from_channel_name(channel) else {
            return Err(format!(
                "unknown channel `{channel}` for `{tool_id}` (known channels: {})",
                known_channels.join(", ")
            ));
        };
        mapped.insert(tool_id.clone(), source);
    }
    Ok(mapped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_config() {
        let config = parse(
            r#"
tools = ["claude", "gemini"]
install_missing = "always"

[channels]
gemini = "brew"
codex = "npm"
"#,
        )
        .unwrap();
        assert_eq!(
            config.tools,
            Some(vec!["claude".to_string(), "gemini".to_string()])
        );
        assert_eq!(config.install_missing, Some(InstallMissing::Always));
        let channels = config.channels.unwrap();
        assert_eq!(channels.get("gemini").map(String::as_str), Some("brew"));
        assert_eq!(channels.get("codex").map(String::as_str), Some("npm"));
    }

    #[test]
    fn empty_text_is_default_config() {
        assert_eq!(parse("").unwrap(), Config::default());
    }

    #[test]
    fn invalid_install_missing_value_is_an_error() {
        assert!(parse(r#"install_missing = "sometimes""#).is_err());
    }

    #[test]
    fn validates_channel_overrides() {
        let mut raw = HashMap::new();
        raw.insert("gemini".to_string(), "npm".to_string());
        raw.insert("codex".to_string(), "brew".to_string());

        let overrides = channel_overrides(Some(&raw), &crate::tools::registry()).unwrap();
        assert_eq!(overrides.get("gemini"), Some(&InstallSource::Npm));
        assert_eq!(overrides.get("codex"), Some(&InstallSource::Brew));
    }

    #[test]
    fn unknown_channel_override_tool_id_is_an_error() {
        let mut raw = HashMap::new();
        raw.insert("no-such-tool".to_string(), "npm".to_string());

        let err = channel_overrides(Some(&raw), &crate::tools::registry()).unwrap_err();
        assert!(err.contains("no-such-tool"), "err was: {err}");
        assert!(err.contains("known ids"), "err was: {err}");
    }

    #[test]
    fn unknown_channel_name_is_an_error() {
        let mut raw = HashMap::new();
        raw.insert("gemini".to_string(), "npn".to_string());

        let err = channel_overrides(Some(&raw), &crate::tools::registry()).unwrap_err();
        assert!(err.contains("npn"), "err was: {err}");
        assert!(err.contains("known channels"), "err was: {err}");
    }

    #[test]
    fn config_path_follows_spec_layout() {
        let path = config_path().expect("home dir exists on dev hosts");
        let rendered = path.to_string_lossy().replace('\\', "/");
        assert!(
            rendered.ends_with("sync-ai-clis/config.toml"),
            "path was: {rendered}"
        );
        #[cfg(unix)]
        assert!(rendered.contains("/.config/"), "path was: {rendered}");
    }
}
