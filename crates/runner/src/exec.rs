//! Running shell commands with the lab environment, a timeout, and optional live output.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use poddojo_core::CommandOutput;

/// Captured output is capped so a runaway command cannot eat the app's memory.
const MAX_OUTPUT: usize = 512 * 1024;

/// The environment every lesson, lab, exam and console command runs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabEnv {
    /// Docker context of the chosen machine (empty: Docker's current context).
    pub docker_context: String,
    /// PodDojo's own kubeconfig (the lab cluster only).
    pub kubeconfig: PathBuf,
    /// Working folder (the lesson's lab folder).
    pub workdir: PathBuf,
    /// Address that reaches ports published on the chosen machine (127.0.0.1 for this computer).
    pub host: String,
    pub cluster: String,
}

impl LabEnv {
    pub fn vars(&self) -> Vec<(String, String)> {
        let mut v = vec![
            ("KUBECONFIG".to_string(), self.kubeconfig.display().to_string()),
            ("PODDOJO_HOST".to_string(), self.host.clone()),
            ("PODDOJO_CLUSTER".to_string(), self.cluster.clone()),
            ("PODDOJO_LAB_DIR".to_string(), self.workdir.display().to_string()),
            ("KIND_CLUSTER_NAME".to_string(), self.cluster.clone()),
            ("PATH".to_string(), search_path()),
        ];
        if !self.docker_context.is_empty() {
            v.push(("DOCKER_CONTEXT".to_string(), self.docker_context.clone()));
        }
        v
    }

    /// `export` lines for a shell (used by `dojo env` and the "Open terminal" button).
    pub fn export_script(&self) -> String {
        let mut s = String::from("unset DOCKER_HOST DOCKER_TLS_VERIFY DOCKER_CERT_PATH\n");
        for (k, v) in self.vars() {
            if k == "PATH" {
                continue;
            }
            s.push_str(&format!("export {k}={}\n", shell_quote(&v)));
        }
        s
    }

    pub fn with_workdir(&self, workdir: PathBuf) -> Self {
        Self { workdir, ..self.clone() }
    }
}

/// Single-quote `s` for POSIX shells.
pub fn shell_quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:=@".contains(c)) {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// `PATH` plus the places Homebrew, Docker Desktop and cargo install tools: apps opened from the
/// Finder start with a bare `PATH` and would not find `docker`, `kubectl`, `kind` or `helm`.
pub fn search_path() -> String {
    let mut parts: Vec<String> =
        std::env::var("PATH").unwrap_or_default().split(':').filter(|p| !p.is_empty()).map(String::from).collect();
    let home = dirs::home_dir().map(|h| h.display().to_string()).unwrap_or_default();
    for extra in [
        "/opt/homebrew/bin".to_string(),
        "/usr/local/bin".to_string(),
        format!("{home}/.docker/bin"),
        "/Applications/Docker.app/Contents/Resources/bin".to_string(),
        format!("{home}/.local/bin"),
        format!("{home}/go/bin"),
        format!("{home}/.cargo/bin"),
        "/usr/bin".to_string(),
        "/bin".to_string(),
        "/usr/sbin".to_string(),
        "/sbin".to_string(),
    ] {
        if !parts.contains(&extra) {
            parts.push(extra);
        }
    }
    parts.join(":")
}

/// Run `cmd` with `/bin/sh -c` in the lab environment.
pub fn run(cmd: &str, env: &LabEnv, timeout: Duration) -> CommandOutput {
    run_live(cmd, env, timeout, None)
}

/// Like [`run`], also appending output to `live` as it arrives (for progress in the UI).
pub fn run_live(cmd: &str, env: &LabEnv, timeout: Duration, live: Option<Arc<Mutex<String>>>) -> CommandOutput {
    let mut command = Command::new("/bin/sh");
    command.arg("-c").arg(cmd);
    command.env_remove("DOCKER_HOST").env_remove("DOCKER_TLS_VERIFY").env_remove("DOCKER_CERT_PATH");
    for (k, v) in env.vars() {
        command.env(k, v);
    }
    // Always run in the lab folder: never fall back to the app's own working directory, where a
    // relative cleanup such as `rm -f app/Dockerfile` would touch the wrong files.
    if let Err(e) = std::fs::create_dir_all(&env.workdir) {
        return CommandOutput::failed_to_start(format!(
            "could not create the lab folder {}: {e}",
            env.workdir.display()
        ));
    }
    command.current_dir(&env.workdir);
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => return CommandOutput::failed_to_start(format!("could not start /bin/sh: {e}")),
    };
    let out_buf = Arc::new(Mutex::new(Vec::new()));
    let err_buf = Arc::new(Mutex::new(Vec::new()));
    let out = child.stdout.take().map(|s| reader(s, out_buf.clone(), live.clone()));
    let err = child.stderr.take().map(|s| reader(s, err_buf.clone(), live));
    let start = Instant::now();
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) if start.elapsed() >= timeout => {
                timed_out = true;
                kill_group(child.id());
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(15)),
            Err(e) => return CommandOutput::failed_to_start(e),
        }
    };
    // A background child (`cmd &`) can outlive the shell and keep the pipes open. Give the readers
    // a short grace period, then kill the whole process group instead of waiting forever.
    let grace = Instant::now()
        + Duration::from_secs(3).min(timeout.saturating_sub(start.elapsed()).max(Duration::from_millis(200)));
    let finished = |h: &Option<std::thread::JoinHandle<()>>| h.as_ref().is_none_or(|h| h.is_finished());
    while !(finished(&out) && finished(&err)) && Instant::now() < grace {
        std::thread::sleep(Duration::from_millis(15));
    }
    if !(finished(&out) && finished(&err)) {
        kill_group(child.id());
    }
    let take = |b: &Arc<Mutex<Vec<u8>>>| b.lock().map(|v| String::from_utf8_lossy(&v).into_owned()).unwrap_or_default();
    CommandOutput { status, stdout: take(&out_buf), stderr: take(&err_buf), timed_out }
}

fn reader(
    mut stream: impl Read + Send + 'static,
    all: Arc<Mutex<Vec<u8>>>,
    live: Option<Arc<Mutex<String>>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match stream.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let chunk = buf.get(..n).unwrap_or_default();
                    if let Ok(mut a) = all.lock()
                        && a.len() < MAX_OUTPUT
                    {
                        a.extend_from_slice(chunk);
                    }
                    if let Some(live) = &live
                        && let Ok(mut l) = live.lock()
                        && l.len() < MAX_OUTPUT
                    {
                        l.push_str(&String::from_utf8_lossy(chunk));
                    }
                }
            }
        }
    })
}

/// Kill the command's whole process group (the shell and whatever it started).
fn kill_group(pid: u32) {
    #[cfg(unix)]
    {
        let _ = Command::new("/bin/kill")
            .args(["-9", "--", &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(unix))]
    let _ = pid;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> LabEnv {
        LabEnv {
            docker_context: "thor".into(),
            kubeconfig: PathBuf::from("/tmp/pd/kubeconfig"),
            workdir: std::env::temp_dir(),
            host: "127.0.0.1".into(),
            cluster: "poddojo".into(),
        }
    }

    #[test]
    fn runs_with_lab_environment() {
        let out = run("echo $DOCKER_CONTEXT $KUBECONFIG; echo oops >&2; exit 3", &env(), Duration::from_secs(5));
        assert_eq!(out.status, Some(3));
        assert_eq!(out.stdout.trim(), "thor /tmp/pd/kubeconfig");
        assert_eq!(out.stderr.trim(), "oops");
    }

    #[test]
    fn times_out_and_kills_children() {
        let t = Instant::now();
        let out = run("sleep 5 & sleep 5; echo done", &env(), Duration::from_millis(300));
        assert!(out.timed_out);
        assert!(t.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn background_children_do_not_hang_the_runner() {
        let t = Instant::now();
        let out = run("sleep 30 & echo started", &env(), Duration::from_secs(20));
        assert_eq!(out.stdout.trim(), "started");
        assert!(t.elapsed() < Duration::from_secs(6));
    }

    #[test]
    fn missing_lab_folder_is_created_not_skipped() {
        let dir = std::env::temp_dir().join(format!("poddojo-exec-{}", std::process::id()));
        let e = LabEnv { workdir: dir.clone(), ..env() };
        let out = run("pwd", &e, Duration::from_secs(5));
        assert!(out.stdout.trim().ends_with(&format!("poddojo-exec-{}", std::process::id())));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn live_output_is_streamed() {
        let live = Arc::new(Mutex::new(String::new()));
        let out = run_live("printf 'a\\nb\\n'", &env(), Duration::from_secs(5), Some(live.clone()));
        assert!(out.success());
        assert_eq!(live.lock().unwrap().as_str(), "a\nb\n");
    }

    #[test]
    fn quoting() {
        assert_eq!(shell_quote("/a/b"), "/a/b");
        assert_eq!(shell_quote("it's here"), r"'it'\''s here'");
        assert!(env().export_script().contains("export DOCKER_CONTEXT=thor"));
    }
}
