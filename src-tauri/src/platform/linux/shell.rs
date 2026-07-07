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

pub fn stop_process(pid: u32, force: bool, expected_name: Option<&str>) -> Result<(), String> {
    verify_process_identity(pid, expected_name)?;

    if force {
        send_signal(pid, "-KILL")?;
    } else {
        send_signal(pid, "-TERM")?;
        std::thread::sleep(std::time::Duration::from_secs(2));
        // Re-verify before escalating: the PID may have been reused by an
        // unrelated process during the grace window.
        if still_matches(pid, expected_name) {
            send_signal(pid, "-KILL")?;
        }
    }
    Ok(())
}

pub fn current_process_name(pid: u32) -> Option<String> {
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let name = comm.trim().to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn verify_process_identity(pid: u32, expected_name: Option<&str>) -> Result<(), String> {
    let Some(expected) = expected_name else {
        return Ok(());
    };

    match current_process_name(pid) {
        Some(name) if crate::platform::shared::process_names_match(&name, expected) => Ok(()),
        Some(name) => Err(format!(
            "PID {pid} now belongs to \"{name}\", not \"{expected}\" — the process list was stale. Refresh and try again."
        )),
        // Already gone: the goal (process stopped) is achieved.
        None => Ok(()),
    }
}

fn still_matches(pid: u32, expected_name: Option<&str>) -> bool {
    match (current_process_name(pid), expected_name) {
        (Some(name), Some(expected)) => {
            crate::platform::shared::process_names_match(&name, expected)
        }
        (Some(_), None) => true,
        (None, _) => false,
    }
}

fn send_signal(pid: u32, signal: &str) -> Result<(), String> {
    let status = std::process::Command::new("kill")
        .arg(signal)
        .arg(pid.to_string())
        .status()
        .map_err(|e| format!("Failed to run kill: {e}"))?;

    if !status.success() {
        return Err(format!("kill {signal} failed for PID {pid}"));
    }

    Ok(())
}
