use std::process::Command;

// In a windows_subsystem = "windows" app, console children (taskkill, clip,
// tasklist, powershell) each flash a visible console window unless spawned
// with CREATE_NO_WINDOW. GUI children (explorer, wt) and the deliberately
// visible terminal must NOT get the flag.
pub(crate) trait NoWindow {
    fn no_window(&mut self) -> &mut Self;
}

impl NoWindow for Command {
    fn no_window(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        self.creation_flags(CREATE_NO_WINDOW)
    }
}

pub fn open_in_file_manager(path: &str) -> Result<(), String> {
    // explorer.exe exits with code 1 even on success, so only spawn errors
    // are treated as failures.
    Command::new("explorer")
        .arg(format!("/select,{path}"))
        .spawn()
        .map_err(|e| format!("Failed to open Explorer: {e}"))?;

    Ok(())
}

pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new("clip")
        .no_window()
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run clip: {e}"))?;

    // clip.exe interprets piped input in the OEM codepage unless it carries a
    // UTF-16LE BOM, which mangles non-ASCII paths — so send UTF-16LE.
    let mut bytes: Vec<u8> = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }

    child
        .stdin
        .as_mut()
        .ok_or_else(|| "Failed to open clip stdin".to_string())?
        .write_all(&bytes)
        .map_err(|e| format!("Failed to write to clip: {e}"))?;

    let status = child.wait().map_err(|e| format!("clip failed: {e}"))?;
    if !status.success() {
        return Err("clip exited with an error".into());
    }

    Ok(())
}

pub fn open_in_terminal(cwd: &str) -> Result<(), String> {
    if let Ok(status) = Command::new("wt").args(["-d", cwd]).status() {
        if status.success() {
            return Ok(());
        }
    }

    Command::new("cmd")
        .current_dir(cwd)
        .spawn()
        .map_err(|e| format!("Failed to open terminal: {e}"))?;

    Ok(())
}

pub fn stop_process(pid: u32, force: bool, expected_name: Option<&str>) -> Result<(), String> {
    verify_process_identity(pid, expected_name)?;

    let mut command = Command::new("taskkill");
    command.no_window().args(["/PID", &pid.to_string()]);
    if force {
        command.arg("/F");
    }

    let status = command
        .status()
        .map_err(|e| format!("Failed to run taskkill: {e}"))?;

    if !status.success() && !force {
        // Console processes reject the graceful WM_CLOSE path outright;
        // re-verify identity before force-killing.
        verify_process_identity(pid, expected_name)?;
        let force_status = Command::new("taskkill")
            .no_window()
            .args(["/F", "/PID", &pid.to_string()])
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

pub fn current_process_name(pid: u32) -> Option<String> {
    let output = Command::new("tasklist")
        .no_window()
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().find(|line| line.starts_with('"'))?;
    let name = line.split('"').nth(1)?.trim().to_string();
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
