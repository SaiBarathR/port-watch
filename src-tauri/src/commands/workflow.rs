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
    super::blocking("Terminal", move || open_in_terminal_blocking(&cwd)).await
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

    let status = launch_editor(binary, cwd)
        .map_err(|e| format!("Failed to run {binary}: {e}. Is it installed and on PATH?"))?;

    if !status.success() {
        return Err(format!("{binary} exited with an error for: {cwd}"));
    }

    Ok(())
}

// The VS Code and Cursor launchers are .cmd scripts, which only cmd.exe can
// run. Naming the script itself lets the standard library quote the folder for
// cmd.exe; a hand-built `cmd /C <editor> <folder>` would run `calc` for a
// folder called `a&calc`.
#[cfg(target_os = "windows")]
fn launch_editor(binary: &str, cwd: &str) -> std::io::Result<std::process::ExitStatus> {
    use crate::platform::shell::NoWindow;
    use std::process::Command;

    match Command::new(format!("{binary}.cmd"))
        .no_window()
        .arg(cwd)
        .status()
    {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Command::new(binary).no_window().arg(cwd).status()
        }
        result => result,
    }
}

#[cfg(not(target_os = "windows"))]
fn launch_editor(binary: &str, cwd: &str) -> std::io::Result<std::process::ExitStatus> {
    std::process::Command::new(binary).arg(cwd).status()
}

#[tauri::command]
pub async fn open_in_editor(cwd: String, editor: String) -> Result<(), String> {
    super::blocking("Editor", move || open_in_editor_blocking(&cwd, &editor)).await
}

#[cfg(test)]
#[cfg(target_os = "windows")]
mod tests {
    use super::*;
    use std::fs;

    // A stand-in editor that records the folder it was handed. Delayed
    // expansion keeps the script itself from re-parsing the argument.
    const FAKE_EDITOR: &str = "@echo off\r\n\
        setlocal EnableDelayedExpansion\r\n\
        set \"ARG=%~1\"\r\n\
        >\"%~dp0out.txt\" echo !ARG!\r\n";

    #[test]
    fn a_folder_name_cannot_inject_a_command() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("fake-editor.cmd"), FAKE_EDITOR).unwrap();
        let folder = dir.path().join("a&echo pwned & (demo)");
        fs::create_dir(&folder).unwrap();
        let folder = folder.to_string_lossy().into_owned();
        let editor = dir.path().join("fake-editor");

        let status = launch_editor(&editor.to_string_lossy(), &folder).unwrap();

        assert!(status.success());
        // With `cmd /C editor <folder>` the editor saw only the part before
        // the first `&`, and the rest ran as a command.
        let seen = fs::read_to_string(dir.path().join("out.txt")).unwrap();
        assert_eq!(seen.trim_end(), folder);
    }
}
