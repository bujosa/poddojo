//! Auto-grading. A check is a shell command run against the learner's Docker host or lab cluster
//! plus an expectation about its output. The runner executes the command; deciding pass or fail is
//! pure and lives here so it can be tested without Docker.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    /// What is being verified, in plain words ("the web container is running").
    pub name: String,
    /// The shell command to run (`/bin/sh -c`), with the lab environment set.
    pub run: String,
    /// Trimmed stdout must equal this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equals: Option<String>,
    /// Stdout must contain this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contains: Option<String>,
    /// Stdout must match this regular expression.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matches: Option<String>,
    /// Required exit status (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<i32>,
    /// Seconds before the command is killed (default 30).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

/// What running a command produced.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CommandOutput {
    /// Exit status; `None` when the command was killed or could not start.
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

impl CommandOutput {
    pub fn failed_to_start(err: impl std::fmt::Display) -> Self {
        Self { status: None, stdout: String::new(), stderr: err.to_string(), timed_out: false }
    }

    pub fn success(&self) -> bool {
        self.status == Some(0)
    }

    /// Stdout and stderr together, the way a terminal shows them.
    pub fn combined(&self) -> String {
        match (self.stdout.trim_end().is_empty(), self.stderr.trim_end().is_empty()) {
            (false, false) => format!("{}\n{}", self.stdout.trim_end(), self.stderr.trim_end()),
            (false, true) => self.stdout.trim_end().to_string(),
            (true, false) => self.stderr.trim_end().to_string(),
            (true, true) => String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
    pub passed: bool,
    /// Why it failed (or what was seen), for the learner.
    pub detail: String,
}

impl Check {
    pub fn timeout_secs(&self) -> u64 {
        self.timeout.unwrap_or(30).clamp(1, 600)
    }

    /// Problems with the check itself (used by content validation).
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.name.trim().is_empty() {
            out.push("check has no name".to_string());
        }
        if self.run.trim().is_empty() {
            out.push(format!("check '{}' has no `run` command", self.name));
        }
        let n = [self.equals.is_some(), self.contains.is_some(), self.matches.is_some()].iter().filter(|b| **b).count();
        if n > 1 {
            out.push(format!("check '{}' sets more than one of equals/contains/matches", self.name));
        }
        if let Some(re) = &self.matches
            && let Err(e) = regex::Regex::new(re)
        {
            out.push(format!("check '{}' has an invalid regex: {e}", self.name));
        }
        if let Some(why) = unsafe_command(&self.run) {
            out.push(format!("check '{}': {why}", self.name));
        }
        out
    }

    /// Decide whether `out` satisfies this check.
    pub fn evaluate(&self, out: &CommandOutput) -> CheckResult {
        let fail = |detail: String| CheckResult { name: self.name.clone(), passed: false, detail };
        if out.timed_out {
            return fail(format!("timed out after {}s", self.timeout_secs()));
        }
        let want = self.exit.unwrap_or(0);
        match out.status {
            None => return fail(format!("could not run: {}", first_line(&out.stderr))),
            Some(code) if code != want => {
                let seen = out.combined();
                return fail(format!("exit status {code} (expected {want}){}", suffix(&seen)));
            }
            Some(_) => {}
        }
        let stdout = out.stdout.trim();
        if let Some(eq) = &self.equals
            && stdout != eq.trim()
        {
            return fail(format!("expected `{}`, got `{}`", eq.trim(), shorten(stdout, 160)));
        }
        if let Some(c) = &self.contains
            && !stdout.contains(c.as_str())
        {
            return fail(format!("output does not contain `{c}`{}", suffix(stdout)));
        }
        if let Some(re) = &self.matches {
            match regex::Regex::new(re) {
                Ok(r) if r.is_match(stdout) => {}
                Ok(_) => return fail(format!("output does not match /{re}/{}", suffix(stdout))),
                Err(e) => return fail(format!("invalid regex in check: {e}")),
            }
        }
        CheckResult { name: self.name.clone(), passed: true, detail: shorten(stdout, 160) }
    }
}

/// Lesson and exam commands run on the learner's real machines (a home server, a NAS). Anything
/// that could touch resources PodDojo did not create, or reach a cluster other than the lab
/// cluster, is rejected at validation time.
pub fn unsafe_command(cmd: &str) -> Option<&'static str> {
    const RULES: &[(&str, &str)] = &[
        ("~/.kube", "must not read ~/.kube: the lab cluster comes from KUBECONFIG"),
        ("--kubeconfig", "must not pick a kubeconfig: the lab cluster comes from KUBECONFIG"),
        ("--context", "must not switch contexts: the machine and cluster come from the environment"),
        ("docker context use", "must not change the user's default Docker context"),
        ("kubectl config use-context", "must not change contexts"),
        ("system prune", "must not prune: other workloads may live on this machine"),
        ("volume prune", "must not prune volumes"),
        ("image prune -a", "must not prune all images"),
        ("container prune", "must not prune containers"),
        ("network prune", "must not prune networks"),
        ("docker ps -q", "must not act on every container: name the dojo- resources"),
        ("docker ps -aq", "must not act on every container: name the dojo- resources"),
        ("rm -rf /", "must not delete outside the lab folder"),
        ("delete ns default", "must not delete the default namespace"),
        ("delete namespace default", "must not delete the default namespace"),
        ("delete ns kube-system", "must not delete kube-system"),
        ("delete namespace kube-system", "must not delete kube-system"),
        ("sudo ", "must not use sudo"),
    ];
    let flat = cmd.split_whitespace().collect::<Vec<_>>().join(" ");
    RULES.iter().find(|(pat, _)| flat.contains(pat)).map(|(_, why)| *why)
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("").trim()
}

fn suffix(seen: &str) -> String {
    if seen.trim().is_empty() { String::new() } else { format!(": `{}`", shorten(seen.trim(), 160)) }
}

/// Cut `s` to at most `max` characters (on a char boundary), adding an ellipsis.
pub fn shorten(s: &str, max: usize) -> String {
    let one_line = s.replace('\n', " ⏎ ");
    if one_line.chars().count() <= max {
        return one_line;
    }
    let mut out: String = one_line.chars().take(max).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(stdout: &str) -> CommandOutput {
        CommandOutput { status: Some(0), stdout: stdout.into(), stderr: String::new(), timed_out: false }
    }

    #[test]
    fn equals_trims() {
        let c = Check { name: "n".into(), run: "x".into(), equals: Some("true".into()), ..Default::default() };
        assert!(c.evaluate(&ok("true\n")).passed);
        assert!(!c.evaluate(&ok("false\n")).passed);
    }

    #[test]
    fn exit_status_is_checked_first() {
        let c = Check { name: "n".into(), run: "x".into(), ..Default::default() };
        let bad = CommandOutput { status: Some(1), stderr: "Error: not found".into(), ..Default::default() };
        let r = c.evaluate(&bad);
        assert!(!r.passed);
        assert!(r.detail.contains("not found"));
        let c = Check { exit: Some(1), ..c };
        assert!(c.evaluate(&bad).passed);
    }

    #[test]
    fn contains_and_matches() {
        let c = Check { name: "n".into(), run: "x".into(), contains: Some("nginx".into()), ..Default::default() };
        assert!(c.evaluate(&ok("image: nginx:1.27")).passed);
        let c = Check { name: "n".into(), run: "x".into(), matches: Some(r"^\d+$".into()), ..Default::default() };
        assert!(c.evaluate(&ok("3")).passed);
        assert!(!c.evaluate(&ok("three")).passed);
    }

    #[test]
    fn timeouts_and_start_failures_fail() {
        let c = Check { name: "n".into(), run: "x".into(), ..Default::default() };
        assert!(!c.evaluate(&CommandOutput { timed_out: true, ..ok("") }).passed);
        assert!(!c.evaluate(&CommandOutput::failed_to_start("no sh")).passed);
    }

    #[test]
    fn problems_are_reported() {
        let c = Check {
            name: "".into(),
            run: "kubectl --kubeconfig ~/.kube/config get pods".into(),
            equals: Some("a".into()),
            matches: Some("(".into()),
            ..Default::default()
        };
        assert_eq!(c.problems().len(), 4);
        assert!(unsafe_command("docker   system prune -af").is_some());
        assert!(unsafe_command("docker rm -f dojo-web").is_none());
    }

    #[test]
    fn shorten_respects_char_boundaries() {
        assert_eq!(shorten("ñññ", 2), "ññ…");
    }
}
