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

pub fn open_in_terminal(cwd: &str) -> Result<(), String> {
    if let Ok(terminal) = std::env::var("TERMINAL") {
        if !terminal.is_empty() {
            let status = std::process::Command::new(&terminal)
                .args(["--working-directory", cwd])
                .status()
                .or_else(|_| {
                    std::process::Command::new(&terminal)
                        .args(["-e", "bash", "--noprofile", "--norc"])
                        .current_dir(cwd)
                        .status()
                })
                .map_err(|e| format!("Failed to launch $TERMINAL: {e}"))?;

            if status.success() {
                return Ok(());
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
        if let Ok(status) = command.status() {
            if status.success() {
                return Ok(());
            }
        }
    }

    Err(format!("Could not open a terminal at: {cwd}"))
}

pub fn stop_process(pid: u32, force: bool) -> Result<(), String> {
    if force {
        send_signal(pid, "-KILL")?;
    } else {
        send_signal(pid, "-TERM")?;
        std::thread::sleep(std::time::Duration::from_secs(2));
        if process_exists(pid) {
            send_signal(pid, "-KILL")?;
        }
    }
    Ok(())
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

fn process_exists(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
