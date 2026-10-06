use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::classifier::{classify as classify_process, SystemKind};
use crate::home::infer_project_root;
use crate::platform::shared::{extract_script_path, run_with_timeout};
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
    // Unix seconds, 0 when unknown. Signed so a clock far in the past cannot
    // fail the whole parse; clamped when building PortProcess.
    #[serde(rename = "startedAt")]
    started_at: i64,
}

// PowerShell alone can take seconds to start on a cold or busy machine, so it
// gets far longer than the Unix tools before it counts as stuck.
const POWERSHELL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub fn scan_listening_ports(include_udp: bool) -> Result<Vec<PortProcess>, String> {
    let listeners = query_listeners(include_udp)?;

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
        // Without a script, that folder is just where the program is
        // installed (per-user VS Code, Cursor, ...), not a project.
        let delete_blocked = script_path.is_none().then(|| {
            "The folder was guessed from where the program is installed, so it may not be a project."
                .to_string()
        });
        let project_root = infer_project_root(if !working_directory.is_empty() {
            &working_directory
        } else {
            script_path.as_deref().unwrap_or(&executable_path)
        });

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
                    id: String::new(),
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
                    // Read the way a stop reads it, so the two agree to
                    // the second; PowerShell's own figure passes through a
                    // local time, which is ambiguous for an hour each year.
                    started_at: super::shell::process_started_at(listener.pid)
                        .unwrap_or(listener.started_at.max(0) as u64),
                    delete_blocked: delete_blocked.clone(),
                };
                classify_process(&mut process);
                process
            });
    }

    Ok(by_pid.into_values().collect())
}

fn normalize_address(address: &str) -> String {
    if address == "0.0.0.0" || address == "::" {
        "*".to_string()
    } else if address.contains(':') && !address.starts_with('[') {
        // Bracket IPv6 addresses to match the macOS/Linux scanners.
        format!("[{address}]")
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

// One PowerShell start per scan, with or without UDP: starting it costs far
// more than either query.
fn scan_script(include_udp: bool) -> String {
    include_str!("scan.ps1").replace(
        "__INCLUDE_UDP__",
        if include_udp { "$true" } else { "$false" },
    )
}

fn query_listeners(include_udp: bool) -> Result<Vec<WindowsListener>, String> {
    let script = scan_script(include_udp);

    use super::shell::NoWindow;
    let output = run_with_timeout(
        std::process::Command::new("powershell").no_window().args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &script,
        ]),
        POWERSHELL_TIMEOUT,
    )
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
        serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse PowerShell JSON: {e}"))
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
    fn scan_script_switches_udp_on_and_off() {
        assert!(scan_script(true).contains("$includeUdp = $true"));
        assert!(scan_script(false).contains("$includeUdp = $false"));
        assert!(!scan_script(true).contains("__INCLUDE_UDP__"));
    }

    #[test]
    fn parses_the_scan_scripts_output() {
        let json = r#"[{"pid":4242,"name":"node.exe","user":"PC\\dev","localAddress":"::","localPort":3000,"executablePath":"C:\\Program Files\\nodejs\\node.exe","commandLine":"node C:\\Users\\dev\\app\\server.js","protocol":"TCP","startedAt":1790000000},{"pid":4,"name":"System","user":"","localAddress":"0.0.0.0","localPort":445,"executablePath":"","commandLine":"","protocol":"TCP","startedAt":0}]"#;
        let listeners: Vec<WindowsListener> = serde_json::from_str(json).unwrap();
        assert_eq!(listeners.len(), 2);
        assert_eq!(listeners[0].pid, 4242);
        assert_eq!(listeners[0].started_at, 1_790_000_000);
        assert_eq!(listeners[1].started_at, 0);
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
