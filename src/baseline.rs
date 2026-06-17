//! Persisted trust baselines for `audit` (design doc 0013): the last accepted
//! content of each tool's remote install script, stored under `data_dir`
//! (clearing it resets change detection — not a disposable cache). One file
//! per tool id holds the full script text, so `audit` compares content
//! directly without a hash dependency.

use std::path::PathBuf;

/// Stores and loads per-tool install-script baselines. The directory is
/// injectable so tests use a temp dir while the binary uses `data_dir`.
pub struct BaselineStore {
    dir: PathBuf,
}

impl BaselineStore {
    pub fn new(dir: PathBuf) -> BaselineStore {
        BaselineStore { dir }
    }

    /// The persistent location the binary uses:
    /// `data_dir`/sync-ai-clis/script-baselines. None when no data dir exists.
    pub fn default_dir() -> Option<PathBuf> {
        dirs::data_dir().map(|d| d.join("sync-ai-clis").join("script-baselines"))
    }

    /// The last accepted script content for `id`, or None if never accepted.
    pub fn load(&self, id: &str) -> Option<String> {
        std::fs::read_to_string(self.dir.join(id)).ok()
    }

    /// Records `content` as the accepted baseline for `id`, creating the
    /// baseline directory if needed.
    pub fn save(&self, id: &str, content: &str) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        std::fs::write(self.dir.join(id), content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A store rooted in a per-test, per-process temp dir, wiped first so each
    /// run starts from a clean slate (the project's temp-dir test pattern).
    fn temp_store(name: &str) -> BaselineStore {
        let dir = std::env::temp_dir().join(format!("sync-audit-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        BaselineStore::new(dir)
    }

    #[test]
    fn save_then_load_round_trips() {
        let store = temp_store("roundtrip");
        store.save("claude", "curl https://x | bash\n").unwrap();
        assert_eq!(
            store.load("claude").as_deref(),
            Some("curl https://x | bash\n")
        );
    }

    #[test]
    fn load_missing_is_none() {
        let store = temp_store("missing");
        assert_eq!(store.load("claude"), None);
    }

    #[test]
    fn save_creates_missing_dir() {
        let store = temp_store("createdir");
        store.save("agy", "body").unwrap();
        assert!(store.load("agy").is_some());
    }
}
