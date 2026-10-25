//! Machines: every Docker context is a place PodDojo can run labs (this computer's Docker
//! Desktop, a Linux box over SSH, a Raspberry Pi…). Create more with
//! `docker context create NAME --docker host=ssh://user@host`.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::exec::{LabEnv, run};
use crate::paths::Paths;
use crate::settings::CLUSTER_NAME;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Kind {
    /// A socket on this computer.
    Local,
    Ssh,
    Tcp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Target {
    pub name: String,
    pub description: String,
    pub endpoint: String,
    /// Docker's current context.
    pub current: bool,
    pub kind: Kind,
    pub error: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawContext {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    docker_endpoint: String,
    #[serde(default)]
    current: bool,
    #[serde(default)]
    error: String,
}

/// Facts about a machine's Docker engine.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineInfo {
    pub server_version: String,
    pub os: String,
    pub arch: String,
    pub cpus: u64,
    pub memory: u64,
    pub containers: u64,
    pub running: u64,
    pub images: u64,
    pub name: String,
}

impl Target {
    fn from_raw(r: RawContext) -> Self {
        let kind = if r.docker_endpoint.starts_with("ssh://") {
            Kind::Ssh
        } else if r.docker_endpoint.starts_with("tcp://") {
            Kind::Tcp
        } else {
            Kind::Local
        };
        Self { name: r.name, description: r.description, endpoint: r.docker_endpoint, current: r.current, kind, error: r.error }
    }

    /// The host part of the endpoint (`ssh://user@10.0.0.11:22` → `10.0.0.11`).
    pub fn endpoint_host(&self) -> Option<String> {
        let rest = self.endpoint.split_once("://")?.1;
        let rest = rest.rsplit_once('@').map_or(rest, |(_, h)| h);
        let host = rest.split('/').next()?;
        let host = if let Some(stripped) = host.strip_prefix('[') {
            stripped.split(']').next()?
        } else {
            host.rsplit_once(':').map_or(host, |(h, _)| h)
        };
        (!host.is_empty()).then(|| host.to_string())
    }

    /// The address that reaches ports published on this machine. SSH aliases (`ssh://pi`) are
    /// resolved with `ssh -G`.
    pub fn address(&self) -> String {
        match self.kind {
            Kind::Local => "127.0.0.1".to_string(),
            Kind::Tcp => self.endpoint_host().unwrap_or_else(|| "127.0.0.1".into()),
            Kind::Ssh => {
                let Some(host) = self.endpoint_host() else { return "127.0.0.1".into() };
                if host.parse::<std::net::IpAddr>().is_ok() {
                    return host;
                }
                resolve_ssh_alias(&host).unwrap_or(host)
            }
        }
    }

    /// The lab environment for this machine with `workdir` as the working folder.
    pub fn env(&self, paths: &Paths, workdir: PathBuf) -> LabEnv {
        LabEnv { docker_context: self.name.clone(), kubeconfig: paths.kubeconfig(), workdir, host: self.address(), cluster: CLUSTER_NAME.into() }
    }

    pub fn is_remote(&self) -> bool {
        self.kind != Kind::Local
    }
}

fn resolve_ssh_alias(alias: &str) -> Option<String> {
    let out = std::process::Command::new("ssh").args(["-G", alias]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines().find_map(|l| l.strip_prefix("hostname ")).map(|h| h.trim().to_string()).filter(|h| !h.is_empty())
}

/// An environment with no context set, for `docker context ls` itself.
fn bare_env(paths: &Paths) -> LabEnv {
    LabEnv { docker_context: String::new(), kubeconfig: paths.kubeconfig(), workdir: paths.root.clone(), host: "127.0.0.1".into(), cluster: CLUSTER_NAME.into() }
}

/// All Docker contexts on this computer.
pub fn list(paths: &Paths) -> Result<Vec<Target>, String> {
    let out = run("docker context ls --format '{{json .}}'", &bare_env(paths), Duration::from_secs(15));
    if !out.success() {
        return Err(if out.status.is_none() && out.stderr.is_empty() {
            "docker is not installed".into()
        } else {
            format!("docker context ls failed: {}", out.combined())
        });
    }
    Ok(parse_contexts(&out.stdout))
}

pub fn parse_contexts(text: &str) -> Vec<Target> {
    text.lines().filter_map(|l| serde_json::from_str::<RawContext>(l.trim()).ok()).map(Target::from_raw).collect()
}

/// The configured target, or Docker's current context when none is set (or it vanished).
pub fn choose(targets: &[Target], wanted: &str) -> Option<Target> {
    targets.iter().find(|t| t.name == wanted).or_else(|| targets.iter().find(|t| t.current)).or_else(|| targets.first()).cloned()
}

/// Ask the machine's engine about itself (`docker info`).
pub fn probe(env: &LabEnv) -> Result<EngineInfo, String> {
    let fmt = "{{json .}}";
    let out = run(&format!("docker info --format '{fmt}'"), env, Duration::from_secs(20));
    if !out.success() {
        return Err(first_error(&out.combined()));
    }
    let v: serde_json::Value = serde_json::from_str(out.stdout.trim()).map_err(|e| e.to_string())?;
    if let Some(errs) = v.get("ServerErrors").and_then(|e| e.as_array())
        && let Some(e) = errs.first().and_then(|e| e.as_str())
    {
        return Err(first_error(e));
    }
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
    let n = |k: &str| v.get(k).and_then(|x| x.as_u64()).unwrap_or_default();
    Ok(EngineInfo {
        server_version: s("ServerVersion"),
        os: s("OperatingSystem"),
        arch: s("Architecture"),
        cpus: n("NCPU"),
        memory: n("MemTotal"),
        containers: n("Containers"),
        running: n("ContainersRunning"),
        images: n("Images"),
        name: s("Name"),
    })
}

fn first_error(text: &str) -> String {
    text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("unknown error").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LS: &str = r#"{"Current":false,"Description":"","DockerEndpoint":"ssh://bujosa@10.0.0.14","Error":"","Name":"macmini"}
{"Current":true,"Description":"Docker Desktop","DockerEndpoint":"unix:///Users/me/.docker/run/docker.sock","Error":"","Name":"desktop-linux"}
{"Current":false,"Description":"","DockerEndpoint":"ssh://pi","Error":"","Name":"pi"}
{"Current":false,"Description":"","DockerEndpoint":"tcp://[fd00::1]:2376","Error":"","Name":"v6"}"#;

    #[test]
    fn parses_contexts_and_hosts() {
        let t = parse_contexts(LS);
        assert_eq!(t.len(), 4);
        assert_eq!(t[0].kind, Kind::Ssh);
        assert_eq!(t[0].address(), "10.0.0.14");
        assert_eq!(t[1].kind, Kind::Local);
        assert_eq!(t[1].address(), "127.0.0.1");
        assert_eq!(t[2].endpoint_host().as_deref(), Some("pi"));
        assert_eq!(t[3].endpoint_host().as_deref(), Some("fd00::1"));
        assert_eq!(choose(&t, "nope").map(|t| t.name), Some("desktop-linux".into()));
        assert_eq!(choose(&t, "pi").map(|t| t.name), Some("pi".into()));
    }
}
