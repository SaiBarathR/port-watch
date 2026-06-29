use tauri::{AppHandle, Manager};

use crate::app_settings::AppSettings;

#[tauri::command]
pub fn set_allow_system_process_actions(app: AppHandle, allow: bool) -> Result<(), String> {
    let settings = app.state::<AppSettings>();
    settings.set_allow_system_process_actions(allow);
    Ok(())
}

#[tauri::command]
pub fn set_use_https_for_localhost(app: AppHandle, use_https: bool) -> Result<(), String> {
    app.state::<AppSettings>()
        .set_use_https_for_localhost(use_https);
    Ok(())
}

#[tauri::command]
pub fn set_preferred_editor(app: AppHandle, editor: String) -> Result<(), String> {
    app.state::<AppSettings>().set_preferred_editor(editor);
    Ok(())
}
