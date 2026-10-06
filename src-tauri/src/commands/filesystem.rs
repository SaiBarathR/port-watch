use tauri::AppHandle;

use crate::platform;
use crate::process_actions::{self, DeleteMode, DeleteRequest};

pub fn open_in_finder_blocking(path: &str) -> Result<(), String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Path is empty".into());
    }

    // Folders only: handing a file to the platform opener would run or open
    // it instead of showing it.
    if !std::path::Path::new(path).is_dir() {
        return Err(format!("Not a folder: {path}"));
    }

    platform::shell::open_in_file_manager(path)
}

#[tauri::command]
pub async fn open_in_finder(path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || open_in_finder_blocking(&path))
        .await
        .map_err(|e| format!("Open task failed: {e}"))?
}

// Stops a process and deletes its project folder as one step, so the folder
// is checked before anything is stopped.
// Async: trash/delete of a large tree (e.g. node_modules) must not block the
// main thread.
#[tauri::command]
pub async fn delete_project(
    app: AppHandle,
    pid: u32,
    expected_name: String,
    path: String,
    mode: DeleteMode,
    confirmation: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        process_actions::delete_project(
            &app,
            pid,
            &DeleteRequest {
                expected_name: &expected_name,
                path: &path,
                mode,
                confirmation: confirmation.as_deref(),
            },
        )
    })
    .await
    .map_err(|e| format!("Delete task failed: {e}"))?
}
