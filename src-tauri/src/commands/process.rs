use tauri::AppHandle;

use crate::process_actions;

// Async so the graceful-stop wait (seconds, when a process ignores SIGTERM)
// runs off the main thread.
// `expected_name` is the name on the row the user acted on; the backend
// refuses if the latest scan knows that PID as something else.
#[tauri::command]
pub async fn stop_process(
    app: AppHandle,
    pid: u32,
    force: Option<bool>,
    expected_name: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        process_actions::stop_process(&app, pid, force == Some(true), expected_name.as_deref())
    })
    .await
    .map_err(|e| format!("Stop task failed: {e}"))?
}
