use std::path::Path;

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("URL is empty".into());
    }

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("URL must start with http:// or https://".into());
    }

    let status = std::process::Command::new("open")
        .arg(url)
        .status()
        .map_err(|e| format!("Failed to open URL: {e}"))?;

    if !status.success() {
        return Err(format!("open command failed for: {url}"));
    }

    Ok(())
}

#[tauri::command]
pub fn open_in_terminal(cwd: String) -> Result<(), String> {
    let cwd = cwd.trim();
    if cwd.is_empty() {
        return Err("Working directory is empty".into());
    }

    if !Path::new(cwd).is_dir() {
        return Err(format!("Directory does not exist: {cwd}"));
    }

    let status = std::process::Command::new("open")
        .args(["-a", "Terminal", cwd])
        .status()
        .map_err(|e| format!("Failed to open Terminal: {e}"))?;

    if !status.success() {
        return Err(format!("Failed to open Terminal at: {cwd}"));
    }

    Ok(())
}

#[tauri::command]
pub fn open_in_editor(cwd: String, editor: String) -> Result<(), String> {
    let cwd = cwd.trim();
    if cwd.is_empty() {
        return Err("Working directory is empty".into());
    }

    if !Path::new(cwd).is_dir() {
        return Err(format!("Directory does not exist: {cwd}"));
    }

    let binary = match editor.as_str() {
        "code" => "code",
        _ => "cursor",
    };

    let status = std::process::Command::new(binary)
        .arg(cwd)
        .status()
        .map_err(|e| format!("Failed to run {binary}: {e}. Is it installed and on PATH?"))?;

    if !status.success() {
        return Err(format!("{binary} exited with an error for: {cwd}"));
    }

    Ok(())
}
