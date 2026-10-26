//! The learner's choices: which machine runs Docker, and how the lab cluster is built.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// The lab cluster's name; its kubeconfig context is `kind-poddojo`.
pub const CLUSTER_NAME: &str = "poddojo";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Docker context name of the machine to use (`docker context ls`). Empty: Docker's current one.
    pub target: String,
    /// kind node image (`kindest/node:v1.xx.y`); empty for kind's default.
    pub node_image: String,
    /// Add a worker node next to the control plane (DaemonSets, scheduling, NetworkPolicies).
    pub worker: bool,
    /// The API server's port on the machine.
    pub api_port: u16,
    /// Host port that reaches NodePort 30080 on the cluster.
    pub nodeport: u16,
    /// UI scale.
    pub zoom: f32,
    pub dark: bool,
    /// Animations (the living logo, transitions). Off = reduced motion.
    pub motion: bool,
    /// Width of the instructions pane in the lab workspace (0: automatic).
    pub lab_split: f32,
    /// Width of the task pane in the exam workspace (0: automatic).
    pub exam_split: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            target: String::new(),
            node_image: String::new(),
            worker: true,
            api_port: 16443,
            nodeport: 30080,
            zoom: 1.0,
            dark: true,
            motion: true,
            lab_split: 0.0,
            exam_split: 0.0,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text =
            serde_json::to_string_pretty(self).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }
}
