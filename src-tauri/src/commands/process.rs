use tauri::AppHandle;

use crate::process_actions::{self, SeenProcess};

// Async so the graceful-stop wait (seconds, when a process ignores SIGTERM)
// runs off the main thread.
// `expected_name` and `expected_started_at` come from the row the user acted
// on; the backend refuses if the latest scan knows that PID as something else.
#[tauri::command]
pub async fn stop_process(
    app: AppHandle,
    pid: u32,
    force: Option<bool>,
    expected_name: Option<String>,
    expected_started_at: Option<u64>,
) -> Result<(), String> {
    super::blocking("Stop", move || {
        process_actions::stop_process(
            &app,
            pid,
            force == Some(true),
            SeenProcess {
                name: expected_name.as_deref(),
                started_at: expected_started_at,
            },
        )
    })
    .await
}
