//! The lab cluster: a kind (Kubernetes-in-Docker) cluster named `poddojo` on the chosen machine,
//! with its kubeconfig in PodDojo's data folder. It is the only cluster PodDojo ever talks to.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::exec::{LabEnv, run, shell_quote};
use crate::paths::Paths;
use crate::settings::{CLUSTER_NAME, Settings};
use crate::targets::Target;

pub const CONTEXT: &str = "kind-poddojo";
/// PodDojo's own Docker network for the cluster nodes (kind would otherwise share `kind`).
pub const NETWORK: &str = "dojo-kind";

/// Create [`NETWORK`] if missing. Docker's automatic address pools run out on machines with many
/// networks ("all predefined address pools have been fully subnetted"), so fall back to explicit,
/// rarely used subnets instead of asking the learner to prune anything.
pub fn ensure_network_command() -> String {
    let candidates =
        ["10.213.7.0/24", "10.219.7.0/24", "10.227.7.0/24", "10.241.7.0/24", "172.40.7.0/24", "192.168.247.0/24"];
    let opts = "--label poddojo=1 -o com.docker.network.bridge.enable_ip_masquerade=true";
    let mut s = format!(
        "docker network inspect {NETWORK} >/dev/null 2>&1 || docker network create {opts} {NETWORK} >/dev/null 2>&1"
    );
    for c in candidates {
        s.push_str(&format!(" || docker network create {opts} --subnet {c} {NETWORK} >/dev/null 2>&1"));
    }
    s.push_str(&format!(" || {{ echo 'could not create the {NETWORK} Docker network' >&2; false; }}"));
    s
}

/// Which machine the lab cluster was created on.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterRecord {
    pub target: String,
    pub created_at: i64,
}

impl ClusterRecord {
    pub fn path(paths: &Paths) -> std::path::PathBuf {
        paths.root.join("cluster.json")
    }

    pub fn load(paths: &Paths) -> Option<Self> {
        std::fs::read_to_string(Self::path(paths)).ok().and_then(|t| serde_json::from_str(&t).ok())
    }

    pub fn save(&self, paths: &Paths) -> std::io::Result<()> {
        let text =
            serde_json::to_string_pretty(self).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::create_dir_all(&paths.root)?;
        std::fs::write(Self::path(paths), text)
    }

    pub fn clear(paths: &Paths) {
        let _ = std::fs::remove_file(Self::path(paths));
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Node {
    pub name: String,
    pub role: String,
    pub ready: bool,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Status {
    /// No lab cluster yet.
    Missing,
    /// The cluster exists but the API server does not answer (machine asleep, containers stopped).
    Unreachable(String),
    Running {
        nodes: Vec<Node>,
    },
}

impl Status {
    pub fn is_running(&self) -> bool {
        matches!(self, Status::Running { .. })
    }

    pub fn version(&self) -> Option<&str> {
        match self {
            Status::Running { nodes } => nodes.first().map(|n| n.version.as_str()),
            _ => None,
        }
    }
}

/// kind's cluster config for this machine. On a remote machine the API server listens on the
/// machine's address so this computer can reach it.
pub fn kind_config(settings: &Settings, target: &Target) -> String {
    let mut s = String::from("kind: Cluster\napiVersion: kind.x-k8s.io/v1alpha4\n");
    s.push_str("networking:\n");
    let address = if target.is_remote() { target.address() } else { "127.0.0.1".into() };
    s.push_str(&format!("  apiServerAddress: \"{address}\"\n  apiServerPort: {}\n", settings.api_port));
    s.push_str("nodes:\n  - role: control-plane\n");
    if !settings.node_image.is_empty() {
        s.push_str(&format!("    image: {}\n", settings.node_image));
    }
    s.push_str("    extraPortMappings:\n");
    s.push_str(&format!(
        "      - containerPort: 30080\n        hostPort: {}\n        protocol: TCP\n",
        settings.nodeport
    ));
    s.push_str("      - containerPort: 30443\n        hostPort: 30443\n        protocol: TCP\n");
    if settings.worker {
        s.push_str("  - role: worker\n");
        if !settings.node_image.is_empty() {
            s.push_str(&format!("    image: {}\n", settings.node_image));
        }
    }
    s
}

/// Write the kind config and return the command that creates the cluster. When a `poddojo` kind
/// cluster already exists on the machine (made by another PodDojo data folder, or the CLI), it is
/// adopted: only its kubeconfig is exported into PodDojo's own file.
pub fn create_command(paths: &Paths, settings: &Settings, target: &Target) -> std::io::Result<String> {
    std::fs::create_dir_all(&paths.root)?;
    let config = paths.root.join("kind-config.yaml");
    std::fs::write(&config, kind_config(settings, target))?;
    let kubeconfig = shell_quote(&paths.kubeconfig().display().to_string());
    Ok(format!(
        "if kind get clusters 2>/dev/null | grep -qx {CLUSTER_NAME}; then \
         echo 'Adopting the existing {CLUSTER_NAME} kind cluster on this machine'; \
         kind export kubeconfig --name {CLUSTER_NAME} --kubeconfig {kubeconfig}; \
         else {} && KIND_EXPERIMENTAL_DOCKER_NETWORK={NETWORK} kind create cluster --name {CLUSTER_NAME} --config {} --kubeconfig {kubeconfig} --wait 180s; fi",
        ensure_network_command(),
        shell_quote(&config.display().to_string()),
    ))
}

pub fn delete_command(paths: &Paths) -> String {
    format!(
        "kind delete cluster --name {CLUSTER_NAME} --kubeconfig {} && (docker network rm {NETWORK} >/dev/null 2>&1 || true)",
        shell_quote(&paths.kubeconfig().display().to_string())
    )
}

/// Refuse to go on unless PodDojo's kubeconfig holds only the lab cluster's context.
pub fn guard(env: &LabEnv) -> Result<(), String> {
    if !env.kubeconfig.exists() {
        return Err("no lab cluster yet: create it on the Cluster page (or `dojo cluster up`)".into());
    }
    let out = run("kubectl config get-contexts -o name", env, Duration::from_secs(10));
    if !out.success() {
        return Err(format!("kubectl could not read the lab kubeconfig: {}", out.combined()));
    }
    let contexts: Vec<&str> = out.stdout.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if contexts.iter().all(|c| *c == CONTEXT) && !contexts.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "refusing to continue: {} holds contexts other than {CONTEXT}: {}",
            env.kubeconfig.display(),
            contexts.join(", ")
        ))
    }
}

pub fn status(env: &LabEnv) -> Status {
    if !env.kubeconfig.exists() {
        return Status::Missing;
    }
    if let Err(e) = guard(env) {
        return Status::Unreachable(e);
    }
    let out = run("kubectl get nodes -o json --request-timeout=8s", env, Duration::from_secs(12));
    if !out.success() {
        return Status::Unreachable(out.combined().lines().last().unwrap_or("API server not reachable").to_string());
    }
    let nodes = parse_nodes(&out.stdout);
    Status::Running { nodes }
}

pub fn parse_nodes(json: &str) -> Vec<Node> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return Vec::new() };
    let items = v.get("items").and_then(|i| i.as_array()).cloned().unwrap_or_default();
    items
        .iter()
        .map(|n| {
            let name = n.pointer("/metadata/name").and_then(|x| x.as_str()).unwrap_or_default().to_string();
            let labels = n.pointer("/metadata/labels").and_then(|l| l.as_object());
            let role = if labels.is_some_and(|l| l.contains_key("node-role.kubernetes.io/control-plane")) {
                "control-plane"
            } else {
                "worker"
            };
            let ready = n
                .pointer("/status/conditions")
                .and_then(|c| c.as_array())
                .is_some_and(|c| c.iter().any(|c| c["type"] == "Ready" && c["status"] == "True"));
            let version =
                n.pointer("/status/nodeInfo/kubeletVersion").and_then(|x| x.as_str()).unwrap_or_default().to_string();
            Node { name, role: role.into(), ready, version }
        })
        .collect()
}

/// The machine to delete the lab cluster from: exactly the one it was created on. If that machine
/// is no longer a Docker context, refuse rather than deleting some other machine's `poddojo`.
pub fn delete_target<'a>(
    targets: &'a [Target],
    record: Option<&ClusterRecord>,
    selected: &str,
) -> Result<&'a Target, String> {
    let want = record.map_or(selected, |r| r.target.as_str());
    targets.iter().find(|t| t.name == want).ok_or_else(|| {
        format!("the lab cluster was created on `{want}`, which is not a Docker context any more: refusing to delete anything")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::targets::parse_contexts;

    #[test]
    fn remote_config_listens_on_machine_address() {
        let t = parse_contexts(r#"{"Current":false,"DockerEndpoint":"ssh://me@10.0.0.11","Name":"thor"}"#);
        let cfg = kind_config(&Settings::default(), &t[0]);
        assert!(cfg.contains("apiServerAddress: \"10.0.0.11\""));
        assert!(cfg.contains("role: worker"));
        let local =
            parse_contexts(r#"{"Current":true,"DockerEndpoint":"unix:///var/run/docker.sock","Name":"default"}"#);
        assert!(kind_config(&Settings::default(), &local[0]).contains("127.0.0.1"));
    }

    #[test]
    fn parses_nodes() {
        let json = r#"{"items":[{"metadata":{"name":"poddojo-control-plane","labels":{"node-role.kubernetes.io/control-plane":""}},
          "status":{"conditions":[{"type":"Ready","status":"True"}],"nodeInfo":{"kubeletVersion":"v1.37.0"}}}]}"#;
        let n = parse_nodes(json);
        assert_eq!(n.len(), 1);
        assert!(n[0].ready);
        assert_eq!(n[0].role, "control-plane");
        assert_eq!(n[0].version, "v1.37.0");
    }
}
