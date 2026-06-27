use std::path::Path;

use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("URL is empty".into());
    }

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("URL must start with http:// or https://".into());
    }

    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("Failed to open URL: {e}"))
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

    crate::platform::shell::open_in_terminal(cwd)
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
