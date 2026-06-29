pub fn open_in_file_manager(path: &str) -> Result<(), String> {
    let status = std::process::Command::new("explorer")
        .arg(format!("/select,{path}"))
        .status()
        .map_err(|e| format!("Failed to open Explorer: {e}"))?;

    if !status.success() {
        return Err(format!("explorer failed for: {path}"));
    }

    Ok(())
}

pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("clip")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run clip: {e}"))?;

    child
        .stdin
        .as_mut()
        .ok_or_else(|| "Failed to open clip stdin".to_string())?
        .write_all(text.as_bytes())
        .map_err(|e| format!("Failed to write to clip: {e}"))?;

    let status = child.wait().map_err(|e| format!("clip failed: {e}"))?;
    if !status.success() {
        return Err("clip exited with an error".into());
    }

    Ok(())
}

pub fn open_in_terminal(cwd: &str) -> Result<(), String> {
    if let Ok(status) = std::process::Command::new("wt").args(["-d", cwd]).status() {
        if status.success() {
            return Ok(());
        }
    }

    std::process::Command::new("cmd")
        .current_dir(cwd)
        .spawn()
        .map_err(|e| format!("Failed to open terminal: {e}"))?;

    Ok(())
}

pub fn stop_process(pid: u32, force: bool) -> Result<(), String> {
    let mut command = std::process::Command::new("taskkill");
    command.args(["/PID", &pid.to_string()]);
    if force {
        command.arg("/F");
    }

    let status = command
        .status()
        .map_err(|e| format!("Failed to run taskkill: {e}"))?;

    if !status.success() && !force {
        command = std::process::Command::new("taskkill");
        command.args(["/F", "/PID", &pid.to_string()]);
        let force_status = command
            .status()
            .map_err(|e| format!("Failed to run taskkill: {e}"))?;
        if !force_status.success() {
            return Err(format!("taskkill failed for PID {pid}"));
        }
        return Ok(());
    }

    if !status.success() {
        return Err(format!("taskkill failed for PID {pid}"));
    }

    Ok(())
}
