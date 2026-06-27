use std::path::Path;

use crate::guards::{validate_delete_path, validate_permanent_delete};
use crate::platform;
use crate::platform::path_validation;

#[tauri::command]
pub fn open_in_finder(path: String) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("Path is empty".into());
    }

    if !Path::new(&path).exists() {
        return Err(format!("Path does not exist: {path}"));
    }

    platform::shell::open_in_file_manager(&path)
}

#[tauri::command]
pub fn move_to_trash(
    path: String,
    is_system_service: bool,
    allow_system_actions: bool,
) -> Result<(), String> {
    path_validation::assert_system_actions_allowed(is_system_service, allow_system_actions)?;
    validate_delete_path(&path)?;

    trash::delete(&path).map_err(|e| format!("Failed to move to Trash: {e}"))
}

#[tauri::command]
pub fn delete_permanently(
    path: String,
    confirmation: String,
    is_system_service: bool,
    allow_system_actions: bool,
) -> Result<(), String> {
    path_validation::assert_system_actions_allowed(is_system_service, allow_system_actions)?;
    validate_permanent_delete(&path, &confirmation)?;

    let path_obj = Path::new(&path);
    if !path_obj.exists() {
        return Err(format!("Path does not exist: {path}"));
    }

    if path_obj.is_dir() {
        std::fs::remove_dir_all(path_obj)
            .map_err(|e| format!("Failed to delete directory: {e}"))
    } else {
        std::fs::remove_file(path_obj)
            .map_err(|e| format!("Failed to delete file: {e}"))
    }
}
