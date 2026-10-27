//! Read-only views of the chosen machine's Docker engine and of the lab cluster, for the
//! explorers. Docker answers in JSON lines (`--format '{{json .}}'`), Kubernetes in `-o json`.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::Serialize;

use crate::cluster;
use crate::exec::{LabEnv, run, shell_quote};

/// One `docker … --format '{{json .}}'` line, as field → text.
pub type Row = BTreeMap<String, String>;

pub fn json_lines(text: &str) -> Vec<Row> {
    text.lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(l.trim()).ok())
        .map(|m| {
            m.into_iter()
                .map(|(k, v)| {
                    let s = match v {
                        serde_json::Value::String(s) => s,
                        other => other.to_string(),
                    };
                    (k, s)
                })
                .collect()
        })
        .collect()
}

fn docker_rows(env: &LabEnv, args: &str) -> Result<Vec<Row>, String> {
    let out = run(&format!("docker {args} --format '{{{{json .}}}}'"), env, Duration::from_secs(30));
    if out.success() { Ok(json_lines(&out.stdout)) } else { Err(out.combined()) }
}

pub fn containers(env: &LabEnv) -> Result<Vec<Row>, String> {
    docker_rows(env, "ps -a --no-trunc")
}

pub fn images(env: &LabEnv) -> Result<Vec<Row>, String> {
    docker_rows(env, "images")
}

pub fn volumes(env: &LabEnv) -> Result<Vec<Row>, String> {
    docker_rows(env, "volume ls")
}

pub fn networks(env: &LabEnv) -> Result<Vec<Row>, String> {
    docker_rows(env, "network ls")
}

/// An image's layers, newest first (`docker history`).
pub fn history(env: &LabEnv, image: &str) -> Result<Vec<Layer>, String> {
    let rows = docker_rows(env, &format!("history --no-trunc {}", shell_quote(image)))?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let size = r.get("Size").cloned().unwrap_or_default();
            Layer {
                created_by: tidy_created_by(r.get("CreatedBy").map_or("", String::as_str)),
                bytes: parse_size(&size),
                size,
                created: r.get("CreatedSince").cloned().unwrap_or_default(),
                comment: r.get("Comment").cloned().unwrap_or_default(),
            }
        })
        .collect())
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Layer {
    pub created_by: String,
    pub size: String,
    pub bytes: u64,
    pub created: String,
    pub comment: String,
}

/// `/bin/sh -c #(nop)  CMD ["nginx"]` → `CMD ["nginx"]`; `RUN /bin/sh -c apk add …` stays.
pub fn tidy_created_by(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix("/bin/sh -c #(nop) ").unwrap_or(s);
    let s = s.strip_prefix("/bin/sh -c ").map(|r| format!("RUN {r}")).unwrap_or_else(|| s.to_string());
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Docker's human sizes (`12.3MB`, `512kB`, `0B`) to bytes (decimal units, like Docker).
pub fn parse_size(s: &str) -> u64 {
    let s = s.trim();
    let split = s.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(s.len());
    let (num, unit) = s.split_at(split);
    let n: f64 = num.parse().unwrap_or(0.0);
    let mult = match unit.trim().to_ascii_uppercase().as_str() {
        "KB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        "TB" => 1e12,
        _ => 1.0,
    };
    (n * mult).max(0.0) as u64
}

pub fn human_bytes(b: u64) -> String {
    let b = b as f64;
    if b >= 1e9 {
        format!("{:.1} GB", b / 1e9)
    } else if b >= 1e6 {
        format!("{:.1} MB", b / 1e6)
    } else if b >= 1e3 {
        format!("{:.0} kB", b / 1e3)
    } else {
        format!("{b:.0} B")
    }
}

// ---------------------------------------------------------------------------------------------
// Kubernetes

/// The resource kinds the cluster explorer lists, CKAD's working set.
/// kubectl's short name for a resource (`""` when it has none): worth learning for exam speed.
pub fn short_name(kind: &str) -> &'static str {
    match kind {
        "pods" => "po",
        "deployments" => "deploy",
        "replicasets" => "rs",
        "statefulsets" => "sts",
        "daemonsets" => "ds",
        "cronjobs" => "cj",
        "services" => "svc",
        "ingresses" => "ing",
        "networkpolicies" => "netpol",
        "configmaps" => "cm",
        "serviceaccounts" => "sa",
        "persistentvolumeclaims" => "pvc",
        "persistentvolumes" => "pv",
        "horizontalpodautoscalers" => "hpa",
        "namespaces" => "ns",
        "nodes" => "no",
        "events" => "ev",
        "resourcequotas" => "quota",
        "limitranges" => "limits",
        "customresourcedefinitions" => "crd",
        "poddisruptionbudgets" => "pdb",
        _ => "",
    }
}

pub const KINDS: &[&str] = &[
    "pods",
    "deployments",
    "replicasets",
    "statefulsets",
    "daemonsets",
    "jobs",
    "cronjobs",
    "services",
    "endpointslices",
    "ingresses",
    "networkpolicies",
    "configmaps",
    "secrets",
    "serviceaccounts",
    "roles",
    "rolebindings",
    "persistentvolumeclaims",
    "persistentvolumes",
    "resourcequotas",
    "limitranges",
    "horizontalpodautoscalers",
    "events",
];

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct KubeObject {
    pub name: String,
    pub namespace: String,
    /// The column a human looks at first (pod phase, ready replicas, service type…).
    pub status: String,
    /// Healthy / unhealthy / neutral for colouring.
    pub health: Health,
    pub detail: String,
    pub age: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub enum Health {
    Good,
    Bad,
    Pending,
    #[default]
    Neutral,
}

pub fn namespaces(env: &LabEnv) -> Result<Vec<String>, String> {
    cluster::guard(env)?;
    let out = run(
        "kubectl get namespaces -o jsonpath='{.items[*].metadata.name}' --request-timeout=8s",
        env,
        Duration::from_secs(12),
    );
    if !out.success() {
        return Err(out.combined());
    }
    Ok(out.stdout.split_whitespace().map(String::from).collect())
}

/// List `kind` in `namespace` (all namespaces when `None`).
pub fn kube_get(env: &LabEnv, kind: &str, namespace: Option<&str>) -> Result<Vec<KubeObject>, String> {
    cluster::guard(env)?;
    let scope = match namespace {
        Some(ns) => format!("-n {}", shell_quote(ns)),
        None => "-A".into(),
    };
    let out = run(
        &format!("kubectl get {} {scope} -o json --request-timeout=10s", shell_quote(kind)),
        env,
        Duration::from_secs(15),
    );
    if !out.success() {
        return Err(out.combined());
    }
    let v: serde_json::Value = serde_json::from_str(&out.stdout).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now();
    Ok(v.get("items")
        .and_then(|i| i.as_array())
        .map(|items| items.iter().map(|o| summarize(kind, o, now)).collect())
        .unwrap_or_default())
}

fn s<'a>(v: &'a serde_json::Value, ptr: &str) -> &'a str {
    v.pointer(ptr).and_then(|x| x.as_str()).unwrap_or_default()
}

fn n(v: &serde_json::Value, ptr: &str) -> i64 {
    v.pointer(ptr).and_then(|x| x.as_i64()).unwrap_or_default()
}

pub fn age(created: &str, now: chrono::DateTime<chrono::Utc>) -> String {
    let Ok(t) = chrono::DateTime::parse_from_rfc3339(created) else { return String::new() };
    let secs = (now - t.with_timezone(&chrono::Utc)).num_seconds().max(0);
    match secs {
        0..60 => format!("{secs}s"),
        60..3600 => format!("{}m", secs / 60),
        3600..86400 => format!("{}h", secs / 3600),
        _ => format!("{}d", secs / 86400),
    }
}

pub fn summarize(kind: &str, o: &serde_json::Value, now: chrono::DateTime<chrono::Utc>) -> KubeObject {
    let mut k = KubeObject {
        name: s(o, "/metadata/name").to_string(),
        namespace: s(o, "/metadata/namespace").to_string(),
        age: age(s(o, "/metadata/creationTimestamp"), now),
        ..Default::default()
    };
    match kind {
        "pods" => {
            let statuses =
                o.pointer("/status/containerStatuses").and_then(|c| c.as_array()).cloned().unwrap_or_default();
            let ready = statuses.iter().filter(|c| c["ready"] == true).count();
            let restarts: i64 = statuses.iter().map(|c| c["restartCount"].as_i64().unwrap_or(0)).sum();
            let waiting = statuses.iter().find_map(|c| c.pointer("/state/waiting/reason").and_then(|r| r.as_str()));
            let terminated =
                statuses.iter().find_map(|c| c.pointer("/state/terminated/reason").and_then(|r| r.as_str()));
            let phase = s(o, "/status/phase");
            let total = o.pointer("/spec/containers").and_then(|c| c.as_array()).map_or(0, Vec::len);
            k.status = waiting.or(terminated).unwrap_or(phase).to_string();
            k.health = match (k.status.as_str(), ready == total && total > 0) {
                ("Running", true) | ("Succeeded" | "Completed", _) => Health::Good,
                ("Pending" | "ContainerCreating" | "PodInitializing", _) | ("Running", false) => Health::Pending,
                _ => Health::Bad,
            };
            k.detail = format!("{ready}/{total} ready · {restarts} restarts · {}", s(o, "/spec/nodeName"));
        }
        "deployments" | "statefulsets" | "replicasets" => {
            let want = n(o, "/spec/replicas");
            let ready = n(o, "/status/readyReplicas");
            k.status = format!("{ready}/{want}");
            k.health = if ready == want { Health::Good } else { Health::Pending };
            let images: Vec<&str> = o
                .pointer("/spec/template/spec/containers")
                .and_then(|c| c.as_array())
                .map(|c| c.iter().filter_map(|c| c["image"].as_str()).collect())
                .unwrap_or_default();
            k.detail = images.join(", ");
        }
        "daemonsets" => {
            let want = n(o, "/status/desiredNumberScheduled");
            let ready = n(o, "/status/numberReady");
            k.status = format!("{ready}/{want}");
            k.health = if ready == want { Health::Good } else { Health::Pending };
        }
        "jobs" => {
            let done = n(o, "/status/succeeded");
            let want = o.pointer("/spec/completions").and_then(|x| x.as_i64()).unwrap_or(1);
            let failed = n(o, "/status/failed");
            k.status = format!("{done}/{want}");
            k.health = if done >= want {
                Health::Good
            } else if failed > 0 {
                Health::Bad
            } else {
                Health::Pending
            };
            if failed > 0 {
                k.detail = format!("{failed} failed");
            }
        }
        "cronjobs" => {
            k.status = s(o, "/spec/schedule").to_string();
            k.detail = format!("last: {}", s(o, "/status/lastScheduleTime"));
        }
        "services" => {
            k.status = s(o, "/spec/type").to_string();
            let ports: Vec<String> = o
                .pointer("/spec/ports")
                .and_then(|p| p.as_array())
                .map(|p| {
                    p.iter()
                        .map(|p| match p["nodePort"].as_i64() {
                            Some(np) => format!("{}:{}/{}", p["port"], np, p["protocol"].as_str().unwrap_or("TCP")),
                            None => format!("{}/{}", p["port"], p["protocol"].as_str().unwrap_or("TCP")),
                        })
                        .collect()
                })
                .unwrap_or_default();
            k.detail = format!("{} · {}", s(o, "/spec/clusterIP"), ports.join(", "));
        }
        "persistentvolumeclaims" | "persistentvolumes" => {
            k.status = s(o, "/status/phase").to_string();
            k.health = if k.status == "Bound" { Health::Good } else { Health::Pending };
            k.detail = s(o, "/spec/storageClassName").to_string();
        }
        "events" => {
            k.status = s(o, "/type").to_string();
            k.health = if k.status == "Warning" { Health::Bad } else { Health::Neutral };
            k.detail =
                format!("{} {}: {}", s(o, "/involvedObject/kind"), s(o, "/involvedObject/name"), s(o, "/message"));
        }
        "configmaps" | "secrets" => {
            let keys = o.get("data").and_then(|d| d.as_object()).map_or(0, |d| d.len());
            k.status = if kind == "secrets" { s(o, "/type").to_string() } else { String::new() };
            k.detail = format!("{keys} key(s)");
        }
        "ingresses" => {
            let hosts: Vec<&str> = o
                .pointer("/spec/rules")
                .and_then(|r| r.as_array())
                .map(|r| r.iter().filter_map(|r| r["host"].as_str()).collect())
                .unwrap_or_default();
            k.status = s(o, "/spec/ingressClassName").to_string();
            k.detail = hosts.join(", ");
        }
        _ => {}
    }
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(parse_size("12.5MB"), 12_500_000);
        assert_eq!(parse_size("0B"), 0);
        assert_eq!(parse_size("1.2kB"), 1200);
        assert_eq!(human_bytes(12_500_000), "12.5 MB");
    }

    #[test]
    fn created_by_is_tidied() {
        assert_eq!(tidy_created_by("/bin/sh -c #(nop)  CMD [\"nginx\"]"), "CMD [\"nginx\"]");
        assert_eq!(tidy_created_by("/bin/sh -c apk add  curl"), "RUN apk add curl");
    }

    #[test]
    fn json_lines_flatten_values() {
        let rows = json_lines("{\"Names\":\"web\",\"Size\":3}\nnot json\n");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["Size"], "3");
    }

    #[test]
    fn pod_summary() {
        let pod: serde_json::Value = serde_json::from_str(
            r#"{"metadata":{"name":"web","namespace":"a","creationTimestamp":"2026-10-25T10:00:00Z"},
            "spec":{"nodeName":"n1","containers":[{"name":"c"}]},
            "status":{"phase":"Running","containerStatuses":[{"ready":false,"restartCount":4,"state":{"waiting":{"reason":"CrashLoopBackOff"}}}]}}"#,
        )
        .unwrap();
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-25T10:05:00Z").unwrap().with_timezone(&chrono::Utc);
        let k = summarize("pods", &pod, now);
        assert_eq!(k.status, "CrashLoopBackOff");
        assert_eq!(k.health, Health::Bad);
        assert_eq!(k.age, "5m");
        assert!(k.detail.contains("4 restarts"));
    }
}
