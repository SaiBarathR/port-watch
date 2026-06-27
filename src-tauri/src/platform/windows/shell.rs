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

pub fn open_in_terminal(cwd: &str) -> Result<(), String> {
    let wt_status = std::process::Command::new("wt")
        .args(["-d", cwd])
        .status();

    if let Ok(status) = wt_status {
        if status.success() {
            return Ok(());
        }
    }

    let status = std::process::Command::new("cmd")
        .args(["/C", "start", "cmd", "/K", &format!("cd /d {cwd}")])
        .status()
        .map_err(|e| format!("Failed to open terminal: {e}"))?;

    if !status.success() {
        return Err(format!("Failed to open terminal at: {cwd}"));
    }

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
