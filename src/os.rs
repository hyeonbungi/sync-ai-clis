//! OS detection: os/arch, Windows build (Win11 gate for Kiro), Linux libc
//! (glibc version vs musl, Kiro variant selection). See SPEC.md §5.1.
//!
//! Parsers are pure functions so every OS branch is testable from any host
//! (SPEC §8.1); the actual probes are thin wrappers that only run on the
//! matching platform.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    MacOs,
    Windows,
    Linux,
}

impl Os {
    /// Maps `std::env::consts::OS` values; unknown platforms are None
    /// (graceful unsupported, SPEC §5.1).
    pub fn from_env_os(s: &str) -> Option<Os> {
        match s {
            "macos" => Some(Os::MacOs),
            "windows" => Some(Os::Windows),
            "linux" => Some(Os::Linux),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Libc {
    Glibc { major: u32, minor: u32 },
    Musl,
}

impl Libc {
    /// True only for glibc at or above the given version (Kiro needs ≥2.34;
    /// musl is never "glibc at least"). SPEC §7.4.
    pub fn glibc_at_least(&self, major: u32, minor: u32) -> bool {
        match self {
            Libc::Glibc { major: m, minor: n } => (*m, *n) >= (major, minor),
            Libc::Musl => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsInfo {
    pub os: Os,
    pub arch: String,
    /// Windows build number when detectable (e.g. 22631). None elsewhere.
    pub windows_build: Option<u32>,
    /// Linux libc flavor when detectable. None elsewhere.
    pub libc: Option<Libc>,
}

impl OsInfo {
    /// Win11 = build 22000 or later (Kiro requires Win11, SPEC §7.4).
    pub fn is_windows_11(&self) -> bool {
        self.windows_build.is_some_and(|build| build >= 22000)
    }

    /// Detects the current platform. None on unsupported OS.
    pub fn detect() -> Option<OsInfo> {
        let os = Os::from_env_os(std::env::consts::OS)?;
        Some(OsInfo {
            os,
            arch: std::env::consts::ARCH.to_string(),
            windows_build: match os {
                Os::Windows => probe_windows_build(),
                _ => None,
            },
            libc: match os {
                Os::Linux => probe_libc(),
                _ => None,
            },
        })
    }
}

/// Parses the build number out of `cmd /c ver` output, e.g.
/// `Microsoft Windows [Version 10.0.22631.4169]` → 22631.
pub fn parse_windows_build(ver_output: &str) -> Option<u32> {
    let marker = "[Version ";
    let start = ver_output.find(marker)? + marker.len();
    let end = ver_output[start..].find(']')? + start;
    ver_output[start..end].split('.').nth(2)?.parse().ok()
}

/// Parses a glibc version from `getconf GNU_LIBC_VERSION` ("glibc 2.39")
/// or `ldd --version` first lines ("ldd (Ubuntu GLIBC 2.39-0ubuntu8.4) 2.39").
pub fn parse_glibc_version(s: &str) -> Option<(u32, u32)> {
    s.split_whitespace().rev().find_map(parse_major_minor)
}

/// "2.39" or "2.39-0ubuntu8.4" → (2, 39). Tokens without a leading
/// `digits.digits` shape are None.
fn parse_major_minor(token: &str) -> Option<(u32, u32)> {
    let mut parts = token.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor_digits: String = parts
        .next()?
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let minor: u32 = minor_digits.parse().ok()?;
    Some((major, minor))
}

/// Classifies `ldd --version` output: musl mentions "musl"; otherwise a
/// glibc version is expected.
pub fn parse_libc_from_ldd(output: &str) -> Option<Libc> {
    if output.to_ascii_lowercase().contains("musl") {
        return Some(Libc::Musl);
    }
    parse_glibc_version(output).map(|(major, minor)| Libc::Glibc { major, minor })
}

/// Runs `cmd /c ver` (Windows only at runtime).
fn probe_windows_build() -> Option<u32> {
    let out = std::process::Command::new("cmd")
        .args(["/c", "ver"])
        .output()
        .ok()?;
    parse_windows_build(&String::from_utf8_lossy(&out.stdout))
}

/// Runs `ldd --version` (musl prints to stderr), falling back to
/// `getconf GNU_LIBC_VERSION` (Linux only at runtime).
fn probe_libc() -> Option<Libc> {
    if let Ok(out) = std::process::Command::new("ldd").arg("--version").output() {
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        if let Some(libc) = parse_libc_from_ldd(&combined) {
            return Some(libc);
        }
    }
    let out = std::process::Command::new("getconf")
        .arg("GNU_LIBC_VERSION")
        .output()
        .ok()?;
    parse_glibc_version(&String::from_utf8_lossy(&out.stdout))
        .map(|(major, minor)| Libc::Glibc { major, minor })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_from_env_maps_known_values() {
        assert_eq!(Os::from_env_os("macos"), Some(Os::MacOs));
        assert_eq!(Os::from_env_os("windows"), Some(Os::Windows));
        assert_eq!(Os::from_env_os("linux"), Some(Os::Linux));
    }

    #[test]
    fn os_from_env_rejects_unknown_platforms() {
        assert_eq!(Os::from_env_os("freebsd"), None);
        assert_eq!(Os::from_env_os(""), None);
    }

    #[test]
    fn windows_11_starts_at_build_22000() {
        let info = |build| OsInfo {
            os: Os::Windows,
            arch: "x86_64".into(),
            windows_build: build,
            libc: None,
        };
        assert!(info(Some(22000)).is_windows_11());
        assert!(info(Some(22631)).is_windows_11());
        assert!(!info(Some(19045)).is_windows_11()); // Windows 10
        assert!(!info(None).is_windows_11()); // unknown build = not Win11
    }

    #[test]
    fn parses_build_from_ver_output() {
        assert_eq!(
            parse_windows_build("Microsoft Windows [Version 10.0.22631.4169]"),
            Some(22631)
        );
        assert_eq!(
            parse_windows_build("\r\nMicrosoft Windows [Version 10.0.19045.5011]\r\n"),
            Some(19045)
        );
        assert_eq!(parse_windows_build("not a version string"), None);
    }

    #[test]
    fn parses_glibc_version_from_getconf_and_ldd() {
        assert_eq!(parse_glibc_version("glibc 2.39"), Some((2, 39)));
        assert_eq!(
            parse_glibc_version("ldd (Ubuntu GLIBC 2.39-0ubuntu8.4) 2.39"),
            Some((2, 39))
        );
        assert_eq!(parse_glibc_version("ldd (GNU libc) 2.31"), Some((2, 31)));
        assert_eq!(parse_glibc_version("no numbers here"), None);
    }

    #[test]
    fn classifies_ldd_output_as_musl_or_glibc() {
        assert_eq!(
            parse_libc_from_ldd("musl libc (x86_64)\nVersion 1.2.4"),
            Some(Libc::Musl)
        );
        assert_eq!(
            parse_libc_from_ldd("ldd (Ubuntu GLIBC 2.39-0ubuntu8.4) 2.39"),
            Some(Libc::Glibc {
                major: 2,
                minor: 39
            })
        );
        assert_eq!(parse_libc_from_ldd(""), None);
    }

    #[test]
    fn glibc_at_least_compares_versions_and_excludes_musl() {
        let glibc = |major, minor| Libc::Glibc { major, minor };
        assert!(glibc(2, 39).glibc_at_least(2, 34));
        assert!(glibc(2, 34).glibc_at_least(2, 34));
        assert!(!glibc(2, 31).glibc_at_least(2, 34));
        assert!(glibc(3, 0).glibc_at_least(2, 34));
        assert!(!Libc::Musl.glibc_at_least(2, 34));
    }

    #[test]
    fn detect_returns_current_platform() {
        // Probe smoke test: read-only detection, no system mutation.
        let info = OsInfo::detect().expect("dev/CI hosts run a supported OS");
        assert_eq!(Some(info.os), Os::from_env_os(std::env::consts::OS));
        assert!(!info.arch.is_empty());
        match info.os {
            Os::Linux => assert!(info.windows_build.is_none()),
            Os::MacOs => {
                assert!(info.windows_build.is_none());
                assert!(info.libc.is_none());
            }
            Os::Windows => assert!(info.libc.is_none()),
        }
    }
}
