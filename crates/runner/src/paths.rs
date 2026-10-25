//! Where PodDojo keeps its files: `~/Library/Application Support/PodDojo` on macOS,
//! `~/.local/share/poddojo` on Linux, or `$PODDOJO_HOME` when set.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn discover() -> Self {
        if let Some(home) = std::env::var_os("PODDOJO_HOME").filter(|h| !h.is_empty()) {
            return Self { root: PathBuf::from(home) };
        }
        let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        let name = if cfg!(target_os = "macos") { "PodDojo" } else { "poddojo" };
        Self { root: base.join(name) }
    }

    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn progress(&self) -> PathBuf {
        self.root.join("progress.json")
    }

    pub fn settings(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    /// The lab cluster's kubeconfig. Never the user's `~/.kube/config`.
    pub fn kubeconfig(&self) -> PathBuf {
        self.root.join("kubeconfig")
    }

    /// The working folder for a lesson's or exam's files (Dockerfiles, manifests…).
    pub fn workdir(&self, id: &str) -> PathBuf {
        let safe: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect();
        self.root.join("labs").join(safe)
    }

    pub fn ensure(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workdir_is_sanitised() {
        let p = Paths::at("/x");
        assert_eq!(p.workdir("../etc"), PathBuf::from("/x/labs/---etc"));
    }
}
