//! The CKAD exam blueprint: the five domains and their weights, from the CNCF's published
//! curriculum (github.com/cncf/curriculum, `CKAD_Curriculum_v1.37.pdf`). Lessons and exam tasks tag
//! themselves with a domain id so progress and mock-exam scores roll up the same way the real exam
//! is scored.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Domain {
    pub id: &'static str,
    pub title: &'static str,
    /// A short name for tight spaces.
    pub short: &'static str,
    /// Share of the real exam, in percent.
    pub weight: u32,
    pub competencies: &'static [&'static str],
}

pub const CKAD_CURRICULUM_VERSION: &str = "v1.37";

pub const DOMAINS: &[Domain] = &[
    Domain {
        id: "design-build",
        title: "Application Design and Build",
        short: "Design & Build",
        weight: 20,
        competencies: &[
            "Define, build and modify container images",
            "Choose and use the right workload resource (Deployment, DaemonSet, CronJob, etc.)",
            "Understand multi-container Pod design patterns (e.g. sidecar, init and others)",
            "Utilize persistent and ephemeral volumes",
        ],
    },
    Domain {
        id: "deployment",
        title: "Application Deployment",
        short: "Deployment",
        weight: 20,
        competencies: &[
            "Use Kubernetes primitives to implement common deployment strategies (e.g. blue/green or canary)",
            "Understand Deployments and how to perform rolling updates",
            "Use the Helm package manager to deploy existing packages",
            "Kustomize",
        ],
    },
    Domain {
        id: "observability",
        title: "Application Observability and Maintenance",
        short: "Observability",
        weight: 15,
        competencies: &[
            "Understand API deprecations",
            "Implement probes and health checks",
            "Use built-in CLI tools to monitor Kubernetes applications",
            "Utilize container logs",
            "Debugging in Kubernetes",
        ],
    },
    Domain {
        id: "config-security",
        title: "Application Environment, Configuration and Security",
        short: "Config & Security",
        weight: 25,
        competencies: &[
            "Discover and use resources that extend Kubernetes (CRD, Operators)",
            "Understand authentication, authorization and admission control",
            "Understand requests, limits, quotas",
            "Define resource requirements",
            "Understand ConfigMaps",
            "Create & consume Secrets",
            "Understand ServiceAccounts",
            "Understand Application Security (SecurityContexts, Capabilities, etc.)",
        ],
    },
    Domain {
        id: "networking",
        title: "Services and Networking",
        short: "Networking",
        weight: 20,
        competencies: &[
            "Demonstrate basic understanding of NetworkPolicies",
            "Provide and troubleshoot access to applications via services",
            "Use Ingress rules to expose applications",
        ],
    },
];

pub fn domain(id: &str) -> Option<&'static Domain> {
    DOMAINS.iter().find(|d| d.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_sum_to_100() {
        assert_eq!(DOMAINS.iter().map(|d| d.weight).sum::<u32>(), 100);
    }
}
