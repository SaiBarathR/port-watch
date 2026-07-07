use crate::scanner::{scan_listening_ports, PortProcess};

// Async: a full lsof/ss/PowerShell scan takes real time and must not block
// the main thread.
#[tauri::command]
pub async fn list_listening_ports(include_udp: Option<bool>) -> Result<Vec<PortProcess>, String> {
    tauri::async_runtime::spawn_blocking(move || scan_listening_ports(include_udp.unwrap_or(false)))
        .await
        .map_err(|e| format!("Scan task failed: {e}"))?
}
