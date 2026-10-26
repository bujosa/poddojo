//! The built-in terminal: an interactive shell on a pseudo-terminal with the lab environment, decoded
//! by a small VT100 screen model that the UI draws. It behaves like a real terminal: `cd`, aliases,
//! variables and history persist between commands, and full-screen programs work (`vim`, `watch`,
//! `kubectl exec -it`, `less`).
//!
//! Safety: bash starts with `--noprofile` and PodDojo's own rc file instead of the learner's
//! dotfiles, so nothing can point `KUBECONFIG` or `DOCKER_CONTEXT` elsewhere before the prompt.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

pub use vt100;

use crate::exec::{LabEnv, search_path, shell_quote};

/// Lines kept above the screen for scrolling back.
const SCROLLBACK: usize = 5000;

/// The screen model, with the callbacks that answer what it cannot answer itself.
pub type Screen = vt100::Parser<Replies>;

/// Collects answers to terminal queries (cursor position, device attributes) and the window title.
#[derive(Default)]
pub struct Replies {
    out: Vec<u8>,
    pub title: String,
}

impl vt100::Callbacks for Replies {
    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        i1: Option<u8>,
        _i2: Option<u8>,
        params: &[&[u16]],
        c: char,
    ) {
        let first = params.first().and_then(|p| p.first()).copied().unwrap_or(0);
        match (i1, c, first) {
            // Where is the cursor? (readline, vim and kubectl's prompts ask.)
            (None, 'n', 6) => {
                let (row, col) = screen.cursor_position();
                self.out.extend_from_slice(format!("\x1b[{};{}R", row + 1, col + 1).as_bytes());
            }
            (None, 'n', 5) => self.out.extend_from_slice(b"\x1b[0n"),
            // What terminal is this? A VT100 with advanced video, like xterm answers.
            (None, 'c', 0) => self.out.extend_from_slice(b"\x1b[?1;2c"),
            (Some(b'>'), 'c', 0) => self.out.extend_from_slice(b"\x1b[>0;10;1c"),
            _ => {}
        }
    }

    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        self.title = String::from_utf8_lossy(title).into_owned();
    }
}

/// A running shell. Dropping it hangs the shell up (like closing a terminal window).
pub struct Session {
    /// The environment it was started with: a different machine or lab folder needs a new shell.
    pub env: LabEnv,
    screen: Arc<Mutex<Screen>>,
    input: Sender<Vec<u8>>,
    alive: Arc<AtomicBool>,
    /// The shell printed something (its prompt): typed-ahead input is safe to send.
    ready: Arc<AtomicBool>,
    last_output: Arc<Mutex<Instant>>,
    resize: Box<dyn Fn(u16, u16) + Send>,
    hangup: Option<Box<dyn FnOnce() + Send>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Session {
    /// Start bash in `env.workdir` with the lab environment on a `rows`×`cols` terminal. `home` holds
    /// the rc file and the shell history; `wake` is called whenever there is new output to draw.
    pub fn start(
        env: &LabEnv,
        machine: &str,
        home: &Path,
        (rows, cols): (u16, u16),
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, String> {
        #[cfg(unix)]
        {
            unix::start(env, machine, home, (rows.max(2), cols.max(10)), Arc::new(wake))
        }
        #[cfg(not(unix))]
        {
            let _ = (env, machine, home, rows, cols, wake);
            Err("The built-in terminal needs macOS or Linux: use Open terminal instead.".into())
        }
    }

    /// Send bytes to the shell (keys, pastes). Never blocks: a writer thread feeds the terminal.
    pub fn write(&self, bytes: impl Into<Vec<u8>>) {
        let bytes = bytes.into();
        if !bytes.is_empty() {
            let _ = self.input.send(bytes);
        }
    }

    /// The screen to draw (locked while the guard lives: keep it short).
    pub fn screen(&self) -> MutexGuard<'_, Screen> {
        lock(&self.screen)
    }

    pub fn resize(&self, rows: u16, cols: u16) {
        let (Some(r), Some(c)) = (std::num::NonZeroU16::new(rows.max(2)), std::num::NonZeroU16::new(cols.max(10)))
        else {
            return;
        };
        lock(&self.screen).set_size(r, c);
        (self.resize)(r.get(), c.get());
    }

    /// The shell is still running (it ends with `exit` or when it is hung up).
    pub fn alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// The shell has printed its first output, so it is reading input.
    pub fn ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    /// When the shell last printed something.
    pub fn last_output(&self) -> Instant {
        *lock(&self.last_output)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(hangup) = self.hangup.take() {
            hangup();
        }
    }
}

/// PodDojo's bash rc file: the lab environment (again, after anything inherited), CKAD habits
/// (`k`, completion, `$do`), a short prompt, then the lab folder.
pub fn rc_script(env: &LabEnv, machine: &str, history: &Path) -> String {
    let machine = if machine.is_empty() { "default" } else { machine };
    let mut s = String::from("# PodDojo lab shell (generated: changes are overwritten)\n");
    s.push_str(&env.export_script());
    s.push_str(&format!("export PATH={}\n", shell_quote(&search_path())));
    s.push_str(&format!("export HISTFILE={}\n", shell_quote(&history.display().to_string())));
    s.push_str("export HISTSIZE=5000 HISTFILESIZE=5000 HISTCONTROL=ignoredups\n");
    s.push_str("shopt -s histappend checkwinsize 2>/dev/null\n");
    s.push_str("export BASH_SILENCE_DEPRECATION_WARNING=1 CLICOLOR=1\n");
    s.push_str("ls --color=auto -d . >/dev/null 2>&1 && alias ls='ls --color=auto'\n");
    s.push_str("alias k=kubectl\n");
    // The two variables most CKAD candidates set up first.
    s.push_str("export do='--dry-run=client -o yaml' now='--force --grace-period=0'\n");
    s.push_str(concat!(
        "if [ \"${BASH_VERSINFO:-0}\" -ge 4 ] && command -v kubectl >/dev/null 2>&1; then\n",
        "  eval \"$(kubectl completion bash 2>/dev/null)\"\n",
        "  complete -o default -F __start_kubectl k 2>/dev/null\n",
        "fi\n",
    ));
    s.push_str(&format!(
        "PS1='\\[\\e[38;5;43m\\]'{}'\\[\\e[0m\\] \\[\\e[38;5;111m\\]\\W\\[\\e[0m\\] \\$ '\n",
        shell_quote(machine)
    ));
    s.push_str(&format!("cd {} 2>/dev/null\n", shell_quote(&env.workdir.display().to_string())));
    s.push_str(&format!(
        "printf '\\033[1;36mPodDojo lab shell\\033[0m  machine \\033[1m%s\\033[0m  cluster \\033[1mkind-%s\\033[0m\\n' {} {}\n",
        shell_quote(machine),
        shell_quote(&env.cluster)
    ));
    s.push_str(
        "printf '\\033[2mkubectl talks only to the lab cluster. k = kubectl, $do = --dry-run=client -o yaml\\033[0m\\n\\n'\n",
    );
    s
}

#[cfg(unix)]
mod unix {
    use std::io::{Read, Write};
    use std::path::Path;
    use std::process::Command;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::channel;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use super::{Replies, SCROLLBACK, Screen, Session, lock, rc_script};
    use crate::exec::LabEnv;

    pub fn start(
        env: &LabEnv,
        machine: &str,
        home: &Path,
        (rows, cols): (u16, u16),
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Session, String> {
        std::fs::create_dir_all(home).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&env.workdir)
            .map_err(|e| format!("could not create the lab folder {}: {e}", env.workdir.display()))?;
        let rc = home.join("lab-shell.bashrc");
        std::fs::write(&rc, rc_script(env, machine, &home.join("lab-shell.history"))).map_err(|e| e.to_string())?;

        let mut cmd = if Path::new("/bin/bash").exists() {
            let mut c = Command::new("/bin/bash");
            c.arg("--noprofile").arg("--rcfile").arg(&rc).arg("-i");
            c
        } else {
            // POSIX sh reads `$ENV` when interactive.
            let mut c = Command::new("/bin/sh");
            c.arg("-i").env("ENV", &rc);
            c
        };
        for k in ["DOCKER_HOST", "DOCKER_TLS_VERIFY", "DOCKER_CERT_PATH", "BASH_ENV", "PROMPT_COMMAND", "PS1"] {
            cmd.env_remove(k);
        }
        for (k, v) in env.vars() {
            cmd.env(k, v);
        }
        cmd.env("TERM", "xterm-256color").env("COLORTERM", "truecolor").env("TERM_PROGRAM", "PodDojo");
        // Apps opened from the Finder get no locale; without one bash mangles UTF-8.
        if std::env::var_os("LANG").is_none() && std::env::var_os("LC_ALL").is_none() {
            cmd.env("LANG", if cfg!(target_os = "macos") { "en_US.UTF-8" } else { "C.UTF-8" });
        }
        cmd.current_dir(&env.workdir);

        let (pty, pts) = pty_process::blocking::open().map_err(|e| format!("could not open a terminal: {e}"))?;
        pty.resize(pty_process::Size::new(rows, cols)).map_err(|e| format!("could not size the terminal: {e}"))?;
        let child = pty_process::blocking::Command::from(cmd)
            .spawn(pts)
            .map_err(|e| format!("could not start the shell: {e}"))?;
        let pty = Arc::new(pty);

        let (Some(r), Some(c)) = (std::num::NonZeroU16::new(rows), std::num::NonZeroU16::new(cols)) else {
            return Err("the terminal has no size".into());
        };
        let screen = Arc::new(Mutex::new(Screen::new_with_callbacks(r, c, SCROLLBACK, Replies::default())));
        let alive = Arc::new(AtomicBool::new(true));
        let ready = Arc::new(AtomicBool::new(false));
        let last_output = Arc::new(Mutex::new(Instant::now()));
        let (tx, rx) = channel::<Vec<u8>>();

        // Writer: keys and pastes go out in order without ever blocking the UI.
        let writer = Arc::clone(&pty);
        std::thread::Builder::new()
            .name("terminal-write".into())
            .spawn(move || {
                for bytes in rx {
                    if (&*writer).write_all(&bytes).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;

        // Reader: feed the screen model, answer its queries, wake the UI.
        let reader = Arc::clone(&pty);
        let (screen_r, alive_r, ready_r, last_r, replies) =
            (Arc::clone(&screen), Arc::clone(&alive), Arc::clone(&ready), Arc::clone(&last_output), tx.clone());
        let wake_r = Arc::clone(&wake);
        std::thread::Builder::new()
            .name("terminal-read".into())
            .spawn(move || {
                let mut buf = vec![0u8; 32 * 1024];
                loop {
                    match (&*reader).read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let answer = {
                                let mut s = lock(&screen_r);
                                s.process(buf.get(..n).unwrap_or_default());
                                std::mem::take(&mut s.callbacks_mut().out)
                            };
                            if !answer.is_empty() {
                                let _ = replies.send(answer);
                            }
                            *lock(&last_r) = Instant::now();
                            ready_r.store(true, Ordering::Relaxed);
                            wake_r();
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                        // EIO: the shell (and everything it started) closed the terminal.
                        Err(_) => break,
                    }
                }
                alive_r.store(false, Ordering::Relaxed);
                wake_r();
            })
            .map_err(|e| e.to_string())?;

        let sizer = Arc::clone(&pty);
        let child = Mutex::new(child);
        Ok(Session {
            env: env.clone(),
            screen,
            input: tx,
            alive,
            ready,
            last_output,
            resize: Box::new(move |rows, cols| {
                let _ = sizer.resize(pty_process::Size::new(rows, cols));
            }),
            hangup: Some(Box::new(move || {
                let mut child = child.into_inner().unwrap_or_else(|e| e.into_inner());
                // SIGHUP, like closing a terminal window: bash passes it on to its jobs and exits.
                let _ = rustix::process::kill_process(
                    rustix::process::Pid::from_child(&child),
                    rustix::process::Signal::HUP,
                );
                // Reap it off the UI thread; force it if it ignores the hangup.
                let _ = std::thread::Builder::new().name("terminal-reap".into()).spawn(move || {
                    for _ in 0..20 {
                        if matches!(child.try_wait(), Ok(Some(_))) {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    let _ = child.kill();
                    let _ = child.wait();
                });
            })),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    fn env(dir: &Path) -> LabEnv {
        LabEnv {
            docker_context: "thor".into(),
            kubeconfig: PathBuf::from("/d/kubeconfig"),
            workdir: dir.join("labs").join("x"),
            host: "10.0.0.11".into(),
            cluster: "poddojo".into(),
        }
    }

    #[test]
    fn rc_sets_lab_environment_after_everything_else() {
        let s = rc_script(&env(Path::new("/d")), "thor", Path::new("/d/hist"));
        assert!(s.contains("export DOCKER_CONTEXT=thor"));
        assert!(s.contains("export KUBECONFIG=/d/kubeconfig"));
        assert!(s.contains("unset DOCKER_HOST"));
        assert!(s.contains("alias k=kubectl"));
        assert!(s.contains("cd /d/labs/x"));
    }

    fn wait_for(session: &Session, needle: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let text = session.screen().screen().contents();
            if text.contains(needle) || Instant::now() > deadline {
                return text;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    #[cfg(unix)]
    #[test]
    fn shell_keeps_state_between_commands() {
        let dir = std::env::temp_dir().join(format!("poddojo-shell-{}", std::process::id()));
        let env = env(&dir);
        let session = Session::start(&env, "thor", &dir, (24, 100), || {}).unwrap();
        session.write("cd .. && export DOJO_X=42\r");
        session.write("echo \"[$DOJO_X:$(basename \"$PWD\"):$KUBECONFIG]\"\r");
        let text = wait_for(&session, "[42:labs:/d/kubeconfig]");
        assert!(text.contains("[42:labs:/d/kubeconfig]"), "{text}");
        session.write("exit\r");
        let deadline = Instant::now() + Duration::from_secs(10);
        while session.alive() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(30));
        }
        assert!(!session.alive());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn answers_cursor_position_queries() {
        let mut p = Screen::new_with_callbacks(
            std::num::NonZeroU16::new(24).unwrap(),
            std::num::NonZeroU16::new(80).unwrap(),
            0,
            Replies::default(),
        );
        p.process(b"ab\x1b[6n");
        assert_eq!(p.callbacks().out, b"\x1b[1;3R");
    }
}
