//! "Is my computer ready?": the tools PodDojo drives and how to install the missing ones.

use std::time::Duration;

use serde::Serialize;

use crate::exec::{LabEnv, run};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tool {
    pub name: &'static str,
    pub why: &'static str,
    pub required: bool,
    /// First line of `--version`, when installed.
    pub version: Option<String>,
    pub install: &'static str,
}

const TOOLS: &[(&str, &str, bool, &str, &str)] = &[
    (
        "docker",
        "the Docker CLI: every Docker lesson, and kind needs it",
        true,
        "docker version --format '{{.Client.Version}}'",
        "brew install --cask docker  (or Docker Engine on Linux)",
    ),
    (
        "kubectl",
        "the Kubernetes CLI: every CKAD lesson and exam",
        true,
        "kubectl version --client -o json | grep -m1 gitVersion",
        "brew install kubectl",
    ),
    ("kind", "creates the lab cluster inside Docker on your chosen machine", true, "kind version", "brew install kind"),
    ("helm", "the Helm lessons (CKAD: Application Deployment)", false, "helm version --short", "brew install helm"),
    (
        "ssh",
        "reaches remote machines (ssh:// Docker contexts)",
        false,
        "ssh -V 2>&1",
        "built into macOS and most Linux distributions",
    ),
];

pub fn check(env: &LabEnv) -> Vec<Tool> {
    TOOLS
        .iter()
        .map(|(name, why, required, version_cmd, install)| {
            let exists = run(&format!("command -v {name}"), env, Duration::from_secs(5)).success();
            let version = exists.then(|| {
                let out = run(version_cmd, env, Duration::from_secs(10));
                let text = out.combined();
                let line = text.lines().next().unwrap_or("installed").trim().trim_end_matches(',').to_string();
                line.replace("\"gitVersion\": ", "").replace('"', "")
            });
            Tool { name, why, required: *required, version, install }
        })
        .collect()
}

pub fn missing_required(tools: &[Tool]) -> Vec<&Tool> {
    tools.iter().filter(|t| t.required && t.version.is_none()).collect()
}
