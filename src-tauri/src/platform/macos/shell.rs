pub fn open_in_file_manager(path: &str) -> Result<(), String> {
    let status = std::process::Command::new("open")
        .arg(path)
        .status()
        .map_err(|e| format!("Failed to open Finder: {e}"))?;

    if !status.success() {
        return Err(format!("open command failed for: {path}"));
    }

    Ok(())
}

pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run pbcopy: {e}"))?;

    child
        .stdin
        .as_mut()
        .ok_or_else(|| "Failed to open pbcopy stdin".to_string())?
        .write_all(text.as_bytes())
        .map_err(|e| format!("Failed to write to pbcopy: {e}"))?;

    let status = child
        .wait()
        .map_err(|e| format!("pbcopy failed: {e}"))?;

    if !status.success() {
        return Err("pbcopy exited with an error".into());
    }

    Ok(())
}

pub fn open_in_terminal(cwd: &str) -> Result<(), String> {
    let status = std::process::Command::new("open")
        .args(["-a", "Terminal", cwd])
        .status()
        .map_err(|e| format!("Failed to open Terminal: {e}"))?;

    if !status.success() {
        return Err(format!("Failed to open Terminal at: {cwd}"));
    }

    Ok(())
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
    let output = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok()?;
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
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
