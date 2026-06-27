use crate::scanner::{scan_listening_ports, PortProcess};

#[tauri::command]
pub fn list_listening_ports(include_udp: Option<bool>) -> Result<Vec<PortProcess>, String> {
    scan_listening_ports(include_udp.unwrap_or(false))
}
