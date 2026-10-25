//! "Open terminal": a real shell with the lab environment, for hands-on work the way the CKAD
//! exam is taken (interactive `kubectl exec -it`, `vim`, `watch`…).

use std::process::Command;

use crate::exec::{LabEnv, shell_quote};

/// The script a new terminal runs: lab environment, a banner, then the user's login shell.
pub fn script(env: &LabEnv, target: &str) -> String {
    let mut s = String::from("#!/bin/sh\n# PodDojo lab shell (generated)\n");
    s.push_str(&env.export_script());
    s.push_str(&format!("export PATH={}\n", shell_quote(&crate::exec::search_path())));
    s.push_str("alias k=kubectl 2>/dev/null\n");
    s.push_str(&format!("cd {} 2>/dev/null\n", shell_quote(&env.workdir.display().to_string())));
    s.push_str("clear\n");
    s.push_str(&format!(
        "printf '\\033[1;36mPodDojo lab shell\\033[0m  machine: %s  cluster: kind-%s\\n' {} {}\n",
        shell_quote(if target.is_empty() { "default" } else { target }),
        shell_quote(&env.cluster)
    ));
    s.push_str("printf 'DOCKER_CONTEXT and KUBECONFIG point at the lab only. Type exit to leave.\\n\\n'\n");
    s.push_str("exec \"${SHELL:-/bin/zsh}\" -i\n");
    s
}

/// Open a terminal window running [`script`].
pub fn open(env: &LabEnv, target: &str, data_root: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(data_root).map_err(|e| e.to_string())?;
    let path = data_root.join("lab-shell.command");
    std::fs::write(&path, script(env, target)).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    let p = path.display().to_string();
    let spawned = if cfg!(target_os = "macos") {
        Command::new("open").args(["-a", "Terminal", &p]).spawn()
    } else {
        Command::new("x-terminal-emulator")
            .args(["-e", &p])
            .spawn()
            .or_else(|_| Command::new("gnome-terminal").args(["--", &p]).spawn())
            .or_else(|_| Command::new("xterm").args(["-e", &p]).spawn())
    };
    spawned
        .map(|_| ())
        .map_err(|e| format!("could not open a terminal: {e}. Run `eval \"$(dojo env)\"` in yours instead."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn script_sets_lab_environment() {
        let env = LabEnv {
            docker_context: "thor".into(),
            kubeconfig: PathBuf::from("/d/kubeconfig"),
            workdir: PathBuf::from("/d/labs/x"),
            host: "10.0.0.11".into(),
            cluster: "poddojo".into(),
        };
        let s = script(&env, "thor");
        assert!(s.contains("export DOCKER_CONTEXT=thor"));
        assert!(s.contains("export KUBECONFIG=/d/kubeconfig"));
        assert!(s.contains("unset DOCKER_HOST"));
    }
}
