use tauri::AppHandle;

use crate::process_actions;
use crate::platform;

#[tauri::command]
pub fn stop_process(
    app: AppHandle,
    pid: u32,
    force: Option<bool>,
) -> Result<(), String> {
    if pid == 0 {
        return Err("Invalid PID".into());
    }

    process_actions::assert_process_action_allowed(&app, pid)?;
    platform::shell::stop_process(pid, force == Some(true))
}
