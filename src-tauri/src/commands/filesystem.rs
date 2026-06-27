use tauri::AppHandle;

use crate::guards::{resolve_delete_path, resolve_permanent_delete};
use crate::platform;
use crate::process_actions;

#[tauri::command]
pub fn open_in_finder(path: String) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Path is empty".into());
    }

    if !std::path::Path::new(path).exists() {
        return Err(format!("Path does not exist: {path}"));
    }

    platform::shell::open_in_file_manager(path)
}

#[tauri::command]
pub fn move_to_trash(app: AppHandle, path: String, pid: u32) -> Result<(), String> {
    process_actions::assert_process_action_allowed(&app, pid)?;
    let canonical = resolve_delete_path(&path)?;

    trash::delete(&canonical).map_err(|e| format!("Failed to move to Trash: {e}"))
}

#[tauri::command]
pub fn delete_permanently(
    app: AppHandle,
    path: String,
    confirmation: String,
    pid: u32,
) -> Result<(), String> {
    process_actions::assert_process_action_allowed(&app, pid)?;
    let canonical = resolve_permanent_delete(&path, &confirmation)?;

    if canonical.is_dir() {
        std::fs::remove_dir_all(&canonical)
            .map_err(|e| format!("Failed to delete directory: {e}"))
    } else {
        std::fs::remove_file(&canonical)
            .map_err(|e| format!("Failed to delete file: {e}"))
    }
}
