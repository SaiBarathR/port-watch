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
