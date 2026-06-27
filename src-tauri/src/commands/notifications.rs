#[tauri::command]
pub fn send_macos_notification(title: String, message: String) -> Result<(), String> {
    let title = escape_applescript(&title);
    let message = escape_applescript(&message);
    let script = format!("display notification \"{message}\" with title \"{title}\"");

    let status = std::process::Command::new("osascript")
        .args(["-e", &script])
        .status()
        .map_err(|e| format!("Failed to run osascript: {e}"))?;

    if !status.success() {
        return Err("osascript notification failed".into());
    }

    Ok(())
}

fn escape_applescript(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
