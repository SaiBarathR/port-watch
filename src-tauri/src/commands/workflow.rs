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

pub fn open_in_terminal_blocking(cwd: &str) -> Result<(), String> {
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
pub async fn open_in_terminal(cwd: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || open_in_terminal_blocking(&cwd))
        .await
        .map_err(|e| format!("Terminal task failed: {e}"))?
}

pub fn open_in_editor_blocking(cwd: &str, editor: &str) -> Result<(), String> {
    let cwd = cwd.trim();
    if cwd.is_empty() {
        return Err("Working directory is empty".into());
    }

    if !Path::new(cwd).is_dir() {
        return Err(format!("Directory does not exist: {cwd}"));
    }

    let binary = match editor {
        "code" => "code",
        _ => "cursor",
    };

    #[cfg(target_os = "windows")]
    {
        use crate::platform::shell::NoWindow;
        // The VS Code/Cursor CLI launchers are .cmd scripts, which
        // CreateProcess cannot spawn directly — go through cmd.exe.
        let status = std::process::Command::new("cmd")
            .no_window()
            .args(["/C", binary, cwd])
            .status()
            .map_err(|e| format!("Failed to run {binary}: {e}. Is it installed and on PATH?"))?;

        if !status.success() {
            return Err(format!("{binary} exited with an error for: {cwd}"));
        }

        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let status = std::process::Command::new(binary)
            .arg(cwd)
            .status()
            .map_err(|e| format!("Failed to run {binary}: {e}. Is it installed and on PATH?"))?;

        if !status.success() {
            return Err(format!("{binary} exited with an error for: {cwd}"));
        }

        Ok(())
    }
}

#[tauri::command]
pub async fn open_in_editor(cwd: String, editor: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || open_in_editor_blocking(&cwd, &editor))
        .await
        .map_err(|e| format!("Editor task failed: {e}"))?
}
