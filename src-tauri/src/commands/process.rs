use crate::platform;
use crate::platform::path_validation;

#[tauri::command]
pub fn stop_process(
    pid: u32,
    force: Option<bool>,
    is_system_service: bool,
    allow_system_actions: bool,
) -> Result<(), String> {
    if pid == 0 {
        return Err("Invalid PID".into());
    }

    path_validation::assert_system_actions_allowed(is_system_service, allow_system_actions)?;
    platform::shell::stop_process(pid, force == Some(true))
}
