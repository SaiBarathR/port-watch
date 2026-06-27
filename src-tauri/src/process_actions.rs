use tauri::{AppHandle, Manager};

use crate::app_settings::AppSettings;
use crate::platform::path_validation;
use crate::poller::PortPoller;

pub fn is_system_service_for_pid(app: &AppHandle, pid: u32) -> bool {
    let poller = app.state::<PortPoller>();
    poller
        .is_system_service(pid)
        .unwrap_or(true)
}

pub fn assert_process_action_allowed(app: &AppHandle, pid: u32) -> Result<(), String> {
    let settings = app.state::<AppSettings>();
    let allow = settings.allow_system_process_actions();
    let is_system = is_system_service_for_pid(app, pid);
    path_validation::assert_system_actions_allowed(is_system, allow)
}
