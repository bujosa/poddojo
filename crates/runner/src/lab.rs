//! Preparing, checking and resetting a lesson's lab or an exam task.

use std::path::Path;
use std::time::Duration;

use poddojo_core::{Check, CheckResult, CommandOutput, Lesson, Needs};

use crate::cluster;
use crate::exec::{LabEnv, run};

/// Run a list of commands, stopping at the first failure. Returns a transcript for the console.
pub fn run_all(commands: &[String], env: &LabEnv, timeout: Duration) -> (bool, String) {
    let mut log = String::new();
    for cmd in commands {
        log.push_str(&format!("$ {cmd}\n"));
        let out = run(cmd, env, timeout);
        let text = out.combined();
        if !text.is_empty() {
            log.push_str(&text);
            log.push('\n');
        }
        if !out.success() {
            log.push_str(&describe_failure(&out));
            return (false, log);
        }
    }
    (true, log)
}

fn describe_failure(out: &CommandOutput) -> String {
    if out.timed_out {
        "✗ timed out\n".into()
    } else {
        format!("✗ exit status {}\n", out.status.map_or("?".into(), |s| s.to_string()))
    }
}

/// Make sure what the lesson needs is there before touching anything.
pub fn preflight(needs: Needs, env: &LabEnv) -> Result<(), String> {
    if needs == Needs::Cluster {
        cluster::guard(env)?;
    }
    Ok(())
}

/// Write the lab's files and run its setup commands.
pub fn prepare(lesson: &Lesson, env: &LabEnv) -> (bool, String) {
    if let Err(e) = preflight(lesson.needs, env) {
        return (false, format!("✗ {e}\n"));
    }
    let Some(lab) = &lesson.lab else { return (true, String::new()) };
    if let Err(e) = write_files(&env.workdir, lab.files.iter()) {
        return (false, format!("✗ could not write lab files: {e}\n"));
    }
    let mut log = String::new();
    if !lab.files.is_empty() {
        log.push_str(&format!("# wrote {} file(s) to {}\n", lab.files.len(), env.workdir.display()));
    }
    let (ok, more) = run_all(&lab.setup, env, Duration::from_secs(300));
    log.push_str(&more);
    (ok, log)
}

/// Run the lab's cleanup commands (errors are reported, not fatal).
pub fn reset(lesson: &Lesson, env: &LabEnv) -> String {
    let Some(lab) = &lesson.lab else { return String::new() };
    let mut log = String::new();
    for cmd in &lab.cleanup {
        log.push_str(&format!("$ {cmd}\n"));
        let out = run(cmd, env, Duration::from_secs(180));
        let text = out.combined();
        if !text.is_empty() {
            log.push_str(&text);
            log.push('\n');
        }
    }
    log
}

pub fn write_files<'a>(dir: &Path, files: impl Iterator<Item = (&'a String, &'a String)>) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, text) in files {
        if name.contains("..") || name.starts_with('/') {
            continue;
        }
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)?;
    }
    Ok(())
}

/// Run checks in order and evaluate each.
pub fn run_checks(checks: &[Check], env: &LabEnv) -> Vec<CheckResult> {
    checks
        .iter()
        .map(|c| {
            let out = run(&c.run, env, Duration::from_secs(c.timeout_secs()));
            c.evaluate(&out)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn env() -> LabEnv {
        LabEnv {
            docker_context: String::new(),
            kubeconfig: PathBuf::from("/nonexistent/kubeconfig"),
            workdir: std::env::temp_dir(),
            host: "127.0.0.1".into(),
            cluster: "poddojo".into(),
        }
    }

    #[test]
    fn run_all_stops_at_first_failure() {
        let (ok, log) = run_all(&["echo one".into(), "false".into(), "echo three".into()], &env(), Duration::from_secs(5));
        assert!(!ok);
        assert!(log.contains("one"));
        assert!(!log.contains("three"));
    }

    #[test]
    fn checks_are_evaluated() {
        let checks = vec![
            Check { name: "a".into(), run: "echo hi".into(), equals: Some("hi".into()), ..Default::default() },
            Check { name: "b".into(), run: "exit 1".into(), ..Default::default() },
        ];
        let r = run_checks(&checks, &env());
        assert!(r[0].passed);
        assert!(!r[1].passed);
    }

    #[test]
    fn cluster_labs_need_the_lab_cluster() {
        assert!(preflight(Needs::Cluster, &env()).is_err());
        assert!(preflight(Needs::Docker, &env()).is_ok());
    }
}
