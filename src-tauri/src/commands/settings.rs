use tauri::{AppHandle, Manager};

use crate::app_settings::AppSettings;

#[tauri::command]
pub fn set_allow_system_process_actions(app: AppHandle, allow: bool) -> Result<(), String> {
    let settings = app.state::<AppSettings>();
    settings.set_allow_system_process_actions(allow);
    Ok(())
}
