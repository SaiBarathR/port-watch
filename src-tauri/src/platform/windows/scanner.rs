use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::classifier::{classify as classify_process, SystemKind};
use crate::home::infer_project_root;
use crate::platform::shared::extract_script_path;
use crate::scanner::{PortBinding, PortProcess};

#[derive(Debug, Deserialize)]
struct WindowsListener {
    pid: u32,
    name: String,
    user: String,
    #[serde(rename = "localAddress")]
    local_address: String,
    #[serde(rename = "localPort")]
    local_port: u16,
    #[serde(rename = "executablePath")]
    executable_path: Option<String>,
    #[serde(rename = "commandLine")]
    command_line: Option<String>,
    protocol: String,
    #[serde(rename = "uptimeSeconds")]
    uptime_seconds: u64,
}

pub fn scan_listening_ports(include_udp: bool) -> Result<Vec<PortProcess>, String> {
    let mut listeners = query_listeners("TCP")?;
    if include_udp {
        listeners.extend(query_listeners("UDP")?);
    }

    let mut by_pid: HashMap<u32, PortProcess> = HashMap::new();

    for listener in listeners {
        let executable_path = listener.executable_path.unwrap_or_default();
        let command_line = listener.command_line.unwrap_or_default();
        let address = normalize_address(&listener.local_address);
        let binding = PortBinding {
            address,
            port: listener.local_port,
            protocol: listener.protocol,
        };

        let script_path = extract_script_path(&command_line, &listener.name);
        let working_directory = infer_working_directory(&executable_path, &script_path);
        let project_root = infer_project_root(
            if !working_directory.is_empty() {
                &working_directory
            } else {
                script_path.as_deref().unwrap_or(&executable_path)
            },
        );

        by_pid
            .entry(listener.pid)
            .and_modify(|process| {
                if !process.ports.iter().any(|b| {
                    b.address == binding.address
                        && b.port == binding.port
                        && b.protocol == binding.protocol
                }) {
                    process.ports.push(binding.clone());
                }
            })
            .or_insert_with(|| {
                let mut process = PortProcess {
                    pid: listener.pid,
                    name: listener.name.clone(),
                    user: listener.user.clone(),
                    ports: vec![binding],
                    executable_path: executable_path.clone(),
                    script_path: script_path.clone(),
                    command_line: command_line.clone(),
                    working_directory: working_directory.clone(),
                    project_root: project_root.clone(),
                    system_kind: SystemKind::User,
                    is_system_service: false,
                    uptime_seconds: listener.uptime_seconds,
                };
                classify_process(&mut process);
                process
            });
    }

    let mut processes: Vec<PortProcess> = by_pid.into_values().collect();
    processes.sort_by(|a, b| {
        a.ports
            .first()
            .map(|p| p.port)
            .unwrap_or(0)
            .cmp(&b.ports.first().map(|p| p.port).unwrap_or(0))
    });

    Ok(processes)
}

fn normalize_address(address: &str) -> String {
    if address == "0.0.0.0" || address == "::" {
        "*".to_string()
    } else {
        address.to_string()
    }
}

fn infer_working_directory(executable_path: &str, script_path: &Option<String>) -> String {
    if let Some(script) = script_path {
        if let Some(parent) = Path::new(script).parent() {
            return parent.to_string_lossy().into_owned();
        }
    }

    if !executable_path.is_empty() {
        if let Some(parent) = Path::new(executable_path).parent() {
            return parent.to_string_lossy().into_owned();
        }
    }

    String::new()
}

fn query_listeners(protocol: &str) -> Result<Vec<WindowsListener>, String> {
    let script = if protocol == "TCP" {
        include_str!("scan_tcp.ps1")
    } else {
        include_str!("scan_udp.ps1")
    };

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|e| format!("Failed to run PowerShell: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "PowerShell scan failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Ok(Vec::new());
    }

    if stdout.starts_with('[') {
        serde_json::from_str(&stdout)
            .map_err(|e| format!("Failed to parse PowerShell JSON: {e}"))
    } else {
        let single: WindowsListener = serde_json::from_str(&stdout)
            .map_err(|e| format!("Failed to parse PowerShell JSON: {e}"))?;
        Ok(vec![single])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_working_directory_from_script_path() {
        assert_eq!(
            infer_working_directory(
                "C:\\Program Files\\nodejs\\node.exe",
                &Some("C:\\Users\\dev\\app\\server.js".to_string()),
            ),
            "C:\\Users\\dev\\app"
        );
    }

    #[test]
    fn normalize_address_wildcard() {
        assert_eq!(normalize_address("0.0.0.0"), "*");
        assert_eq!(normalize_address("127.0.0.1"), "127.0.0.1");
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn scan_listening_ports_live() {
        scan_listening_ports(false).expect("scan should succeed on Windows");
    }
}
