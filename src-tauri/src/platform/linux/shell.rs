use crate::platform::identity::{LiveProcess, Probe};
use crate::platform::unix;
use crate::scanner::ProcessIdentity;

pub fn open_in_file_manager(path: &str) -> Result<(), String> {
    let status = std::process::Command::new("xdg-open")
        .arg(path)
        .status()
        .map_err(|e| format!("Failed to open file manager: {e}"))?;

    if !status.success() {
        return Err(format!("xdg-open failed for: {path}"));
    }

    Ok(())
}

pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let candidates: [(&str, &[&str]); 3] = [
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
    ];

    for (cmd, args) in candidates {
        let mut child = match Command::new(cmd).args(args).stdin(Stdio::piped()).spawn() {
            Ok(child) => child,
            Err(_) => continue,
        };

        if let Some(stdin) = child.stdin.as_mut() {
            if stdin.write_all(text.as_bytes()).is_err() {
                continue;
            }
        }

        if let Ok(status) = child.wait() {
            if status.success() {
                return Ok(());
            }
        }
    }

    Err("No clipboard utility found (install wl-clipboard, xclip, or xsel)".into())
}

pub fn open_in_terminal(cwd: &str) -> Result<(), String> {
    // spawn() rather than status(): terminals like xterm/konsole stay in the
    // foreground, so waiting on them would block until the window is closed.
    // A failed spawn (missing binary, e.g. a stale $TERMINAL) falls through
    // to the next candidate.
    if let Ok(terminal) = std::env::var("TERMINAL") {
        if !terminal.is_empty() {
            // Not every terminal supports --working-directory; give it a
            // moment and if it exited with an error, retry with the cwd
            // inherited instead. Runs on a worker thread, so the wait is fine.
            if let Ok(mut child) = std::process::Command::new(&terminal)
                .args(["--working-directory", cwd])
                .spawn()
            {
                std::thread::sleep(std::time::Duration::from_millis(500));
                match child.try_wait() {
                    Ok(Some(status)) if !status.success() => {
                        if std::process::Command::new(&terminal)
                            .current_dir(cwd)
                            .spawn()
                            .is_ok()
                        {
                            return Ok(());
                        }
                    }
                    // Still running (or exited cleanly): the flag was accepted.
                    _ => return Ok(()),
                }
            }
        }
    }

    let attempts: [(&str, Vec<&str>); 4] = [
        ("xdg-terminal-exec", vec!["--dir", cwd]),
        ("gnome-terminal", vec!["--working-directory", cwd]),
        ("konsole", vec!["--workdir", cwd]),
        ("xterm", vec!["-e", "bash", "--noprofile", "--norc"]),
    ];

    for (cmd, args) in attempts {
        let mut command = std::process::Command::new(cmd);
        command.args(args);
        if cmd == "xterm" {
            command.current_dir(cwd);
        }
        if command.spawn().is_ok() {
            return Ok(());
        }
    }

    Err(format!("Could not open a terminal at: {cwd}"))
}

pub fn stop_process(
    pid: u32,
    force: bool,
    expected: Option<&ProcessIdentity>,
) -> Result<(), String> {
    unix::stop_process(&probe(), pid, force, expected)
}

pub fn is_running(pid: u32) -> bool {
    live_process(pid) != LiveProcess::Gone
}

// Both names are the kernel's 15-character `comm`, but the comparison stays
// as tolerant as it has been.
pub fn probe() -> Probe {
    Probe {
        live: live_process,
        names_match: crate::platform::shared::process_names_match,
        // Both readings come from /proc/<pid>/stat.
        start_slack: 0,
    }
}

fn live_process(pid: u32) -> LiveProcess {
    let proc_dir = std::path::PathBuf::from(format!("/proc/{pid}"));
    // Only a missing /proc/<pid> says the process is gone.
    let Ok(stat) = std::fs::read_to_string(proc_dir.join("stat")) else {
        return LiveProcess::Gone;
    };
    if matches!(parse_state(&stat), Some('Z' | 'X')) {
        return LiveProcess::Gone;
    }

    // State and start time come from that one read, so they describe the
    // same process. The name is a second read; if it fails, the process is
    // reported without one rather than as gone.
    let name = std::fs::read_to_string(proc_dir.join("comm"))
        .ok()
        .map(|comm| comm.trim().to_string())
        .filter(|name| !name.is_empty());
    LiveProcess::Running {
        name,
        started_at: super::scanner::started_at_from_stat(&stat),
    }
}

// The state is the field after the name, which is parenthesised and may
// itself contain spaces and parentheses. Z (zombie) and X (dead) have exited.
fn parse_state(proc_pid_stat: &str) -> Option<char> {
    proc_pid_stat
        .rsplit_once(')')
        .and_then(|(_, rest)| rest.split_whitespace().next())
        .and_then(|state| state.chars().next())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_state_reads_the_field_after_the_name() {
        assert_eq!(parse_state("42 (sleep) S 1 42 42"), Some('S'));
        assert_eq!(parse_state("42 (tmux: server (1)) Z 1 42"), Some('Z'));
        assert_eq!(parse_state(""), None);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn this_process_is_running_under_its_own_name_and_start_time() {
        let pid = std::process::id();
        let LiveProcess::Running { name, started_at } = live_process(pid) else {
            panic!("the test process should be running");
        };
        let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).unwrap();
        assert_eq!(name.as_deref(), Some(comm.trim()));
        assert!(started_at > 0);
        assert!(is_running(pid));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn a_pid_that_does_not_exist_is_gone() {
        let mut child = std::process::Command::new("true")
            .spawn()
            .expect("spawn true");
        let pid = child.id();
        child.wait().expect("wait");
        assert_eq!(live_process(pid), LiveProcess::Gone);
    }
}
