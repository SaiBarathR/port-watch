use tauri::AppHandle;

use crate::guards::{resolve_delete_path, resolve_permanent_delete};
use crate::platform;
use crate::process_actions;

pub fn open_in_finder_blocking(path: &str) -> Result<(), String> {
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
pub async fn open_in_finder(path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || open_in_finder_blocking(&path))
        .await
        .map_err(|e| format!("Open task failed: {e}"))?
}

// Async: trash/delete of a large tree (e.g. node_modules) must not block the
// main thread.
#[tauri::command]
pub async fn move_to_trash(app: AppHandle, path: String, pid: u32) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        process_actions::assert_process_action_allowed(&app, pid)?;
        let canonical = resolve_delete_path(&path)?;

        trash::delete(&canonical).map_err(|e| format!("Failed to move to Trash: {e}"))
    })
    .await
    .map_err(|e| format!("Trash task failed: {e}"))?
}

#[tauri::command]
pub async fn delete_permanently(
    app: AppHandle,
    path: String,
    confirmation: String,
    pid: u32,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        process_actions::assert_process_action_allowed(&app, pid)?;
        let canonical = resolve_permanent_delete(&path, &confirmation)?;

        if canonical.is_dir() {
            std::fs::remove_dir_all(&canonical)
                .map_err(|e| format!("Failed to delete directory: {e}"))
        } else {
            std::fs::remove_file(&canonical).map_err(|e| format!("Failed to delete file: {e}"))
        }
    })
    .await
    .map_err(|e| format!("Delete task failed: {e}"))?
}
