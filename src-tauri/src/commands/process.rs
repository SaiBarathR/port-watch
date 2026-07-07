use tauri::AppHandle;

use crate::platform;
use crate::process_actions;

pub fn stop_process_blocking(
    app: &AppHandle,
    pid: u32,
    force: bool,
    expected_name: Option<&str>,
) -> Result<(), String> {
    if pid == 0 {
        return Err("Invalid PID".into());
    }

    process_actions::assert_process_action_allowed(app, pid)?;
    platform::shell::stop_process(pid, force, expected_name)
}

// Async so the up-to-2s graceful-stop window runs off the main thread.
// `expected_name` lets the backend refuse to kill a PID that has been
// reused by a different process since the caller's snapshot.
#[tauri::command]
pub async fn stop_process(
    app: AppHandle,
    pid: u32,
    force: Option<bool>,
    expected_name: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        stop_process_blocking(&app, pid, force == Some(true), expected_name.as_deref())
    })
    .await
    .map_err(|e| format!("Stop task failed: {e}"))?
}
