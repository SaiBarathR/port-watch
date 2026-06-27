use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

#[tauri::command]
pub fn send_notification(app: AppHandle, title: String, message: String) -> Result<(), String> {
    app.notification()
        .builder()
        .title(title)
        .body(message)
        .show()
        .map_err(|e| format!("Failed to send notification: {e}"))
}
