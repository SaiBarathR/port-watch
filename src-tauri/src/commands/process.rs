use std::thread;
use std::time::Duration;

#[tauri::command]
pub fn stop_process(pid: u32, force: Option<bool>) -> Result<(), String> {
    if pid == 0 {
        return Err("Invalid PID".into());
    }

    if force != Some(true) {
        send_signal(pid, "-TERM")?;
        thread::sleep(Duration::from_secs(2));

        if process_exists(pid) {
            send_signal(pid, "-KILL")?;
        }
    } else {
        send_signal(pid, "-KILL")?;
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
