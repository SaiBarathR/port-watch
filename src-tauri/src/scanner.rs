use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::classifier::{classify, SystemKind};
use crate::home::infer_project_root;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortBinding {
    pub address: String,
    pub port: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortProcess {
    pub pid: u32,
    pub name: String,
    pub user: String,
    pub ports: Vec<PortBinding>,
    pub executable_path: String,
    pub script_path: Option<String>,
    pub command_line: String,
    pub working_directory: String,
    pub project_root: String,
    pub system_kind: SystemKind,
    pub is_system_service: bool,
    pub uptime_seconds: u64,
}

#[derive(Debug, Default, Clone)]
struct PsInfo {
    user: String,
    command_line: String,
    uptime_seconds: u64,
}

#[derive(Debug, Default, Clone)]
struct ProcessPaths {
    working_directory: String,
    executable_path: String,
}

#[derive(Debug, Default)]
struct LsofRecord {
    pid: Option<u32>,
    name: Option<String>,
    bindings: Vec<PortBinding>,
}

const PS_BATCH_SIZE: usize = 100;
const LSOF_BATCH_SIZE: usize = 50;

pub fn scan_listening_ports(include_udp: bool) -> Result<Vec<PortProcess>, String> {
    let mut records = run_lsof_tcp()?;
    if include_udp {
        records.extend(run_lsof_udp()?);
    }

    let records = merge_by_pid(records);
    let pids: Vec<u32> = records
        .iter()
        .filter_map(|record| {
            if record.bindings.is_empty() {
                None
            } else {
                record.pid
            }
        })
        .collect();

    let ps_info = fetch_ps_info_batch(&pids)?;
    let paths = fetch_lsof_paths_batch(&pids);

    let mut processes: Vec<PortProcess> = Vec::new();

    for record in records {
        let Some(pid) = record.pid else {
            continue;
        };
        let Some(name) = record.name else {
            continue;
        };
        if record.bindings.is_empty() {
            continue;
        }

        let ps = ps_info.get(&pid).cloned().unwrap_or_default();
        let path_info = paths.get(&pid).cloned().unwrap_or_default();
        let script_path = extract_script_path(&ps.command_line, &name);
        let project_root = infer_project_root(
            if !path_info.working_directory.is_empty() {
                &path_info.working_directory
            } else {
                script_path
                    .as_deref()
                    .unwrap_or(&path_info.executable_path)
            },
        );

        let mut process = PortProcess {
            pid,
            name,
            user: ps.user,
            ports: record.bindings,
            executable_path: path_info.executable_path,
            script_path,
            command_line: ps.command_line,
            working_directory: path_info.working_directory,
            project_root,
            system_kind: SystemKind::User,
            is_system_service: false,
            uptime_seconds: ps.uptime_seconds,
        };

        classify(&mut process);
        processes.push(process);
    }

    processes.sort_by(|a, b| {
        a.ports
            .first()
            .map(|p| p.port)
            .unwrap_or(0)
            .cmp(&b.ports.first().map(|p| p.port).unwrap_or(0))
    });

    Ok(processes)
}

fn run_lsof_tcp() -> Result<Vec<LsofRecord>, String> {
    let output = std::process::Command::new("lsof")
        .args(["-iTCP", "-sTCP:LISTEN", "-n", "-P", "-F", "pcn"])
        .output()
        .map_err(|e| format!("Failed to run lsof: {e}"))?;

    if !output.status.success() && output.stdout.is_empty() {
        return Err(format!(
            "lsof exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_lsof_output(&stdout, "TCP"))
}

fn run_lsof_udp() -> Result<Vec<LsofRecord>, String> {
    let output = std::process::Command::new("lsof")
        .args(["-iUDP", "-n", "-P", "-F", "pcn"])
        .output()
        .map_err(|e| format!("Failed to run lsof for UDP: {e}"))?;

    if !output.status.success() && output.stdout.is_empty() {
        return Err(format!(
            "lsof UDP exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_lsof_output(&stdout, "UDP"))
}

fn parse_lsof_output(stdout: &str, protocol: &str) -> Vec<LsofRecord> {
    let mut records: Vec<LsofRecord> = Vec::new();
    let mut current = LsofRecord::default();

    for line in stdout.lines() {
        if line.is_empty() {
            continue;
        }

        let (tag, value) = line.split_at(1);
        match tag {
            "p" => {
                if current.pid.is_some() {
                    records.push(current);
                    current = LsofRecord::default();
                }
                current.pid = value.parse().ok();
            }
            "c" => {
                current.name = Some(value.to_string());
            }
            "n" => {
                if let Some(binding) = parse_network_name(value, protocol) {
                    current.bindings.push(binding);
                }
            }
            _ => {}
        }
    }

    if current.pid.is_some() {
        records.push(current);
    }

    records
}

fn merge_by_pid(records: Vec<LsofRecord>) -> Vec<LsofRecord> {
    let mut by_pid: HashMap<u32, LsofRecord> = HashMap::new();

    for record in records {
        let Some(pid) = record.pid else {
            continue;
        };

        by_pid
            .entry(pid)
            .and_modify(|existing| {
                if existing.name.is_none() {
                    existing.name = record.name.clone();
                }
                for binding in &record.bindings {
                    if !existing.bindings.iter().any(|b| {
                        b.address == binding.address
                            && b.port == binding.port
                            && b.protocol == binding.protocol
                    }) {
                        existing.bindings.push(binding.clone());
                    }
                }
            })
            .or_insert(record);
    }

    by_pid.into_values().collect()
}

fn parse_network_name(value: &str, protocol: &str) -> Option<PortBinding> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    let (address, port_str) = if value.starts_with('[') {
        let end = value.find("]:")?;
        let address = value[..=end].to_string();
        let port_str = &value[end + 2..];
        (address, port_str)
    } else {
        let (addr, port) = value.rsplit_once(':')?;
        (addr.to_string(), port)
    };

    let port: u16 = port_str.parse().ok()?;

    Some(PortBinding {
        address,
        port,
        protocol: protocol.to_string(),
    })
}

fn fetch_ps_info_batch(pids: &[u32]) -> Result<HashMap<u32, PsInfo>, String> {
    let mut result = HashMap::new();
    if pids.is_empty() {
        return Ok(result);
    }

    for chunk in pids.chunks(PS_BATCH_SIZE) {
        let pid_list = chunk
            .iter()
            .map(|pid| pid.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let output = std::process::Command::new("ps")
            .args([
                "-ww",
                "-p",
                &pid_list,
                "-o",
                "pid=,user=,etimes=,command=",
            ])
            .output()
            .map_err(|e| format!("Failed to run ps: {e}"))?;

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let mut parts = line.splitn(4, char::is_whitespace);
            let Some(pid_str) = parts.next() else {
                continue;
            };
            let Ok(pid) = pid_str.parse::<u32>() else {
                continue;
            };
            let user = parts.next().unwrap_or("").trim().to_string();
            let etimes = parts
                .next()
                .unwrap_or("0")
                .trim()
                .parse::<u64>()
                .unwrap_or(0);
            let command = parts.next().unwrap_or("").trim().to_string();

            result.insert(
                pid,
                PsInfo {
                    user,
                    command_line: command,
                    uptime_seconds: etimes,
                },
            );
        }
    }

    Ok(result)
}

fn fetch_lsof_paths_batch(pids: &[u32]) -> HashMap<u32, ProcessPaths> {
    let mut result = HashMap::new();
    if pids.is_empty() {
        return result;
    }

    for chunk in pids.chunks(LSOF_BATCH_SIZE) {
        let pid_list = chunk
            .iter()
            .map(|pid| pid.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let output = match std::process::Command::new("lsof")
            .args(["-a", "-p", &pid_list, "-d", "cwd,txt", "-Fn"])
            .output()
        {
            Ok(output) => output,
            Err(_) => continue,
        };

        let mut current_pid: Option<u32> = None;
        let mut current_fd: Option<&str> = None;

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if line.is_empty() {
                continue;
            }

            let (tag, value) = line.split_at(1);
            match tag {
                "p" => {
                    current_pid = value.parse().ok();
                    current_fd = None;
                }
                "f" => {
                    current_fd = Some(value);
                }
                "n" => {
                    let Some(pid) = current_pid else {
                        continue;
                    };
                    let entry = result.entry(pid).or_default();
                    match current_fd {
                        Some("cwd") => entry.working_directory = value.to_string(),
                        Some("txt") => entry.executable_path = value.to_string(),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    result
}

fn extract_script_path(command_line: &str, process_name: &str) -> Option<String> {
    if command_line.is_empty() {
        return None;
    }

    let interpreters = [
        "python", "python3", "node", "nodejs", "bun", "ruby", "perl", "php", "java",
    ];

    let name_lower = process_name.to_lowercase();
    let is_interpreter = interpreters.iter().any(|i| name_lower.contains(i));

    let tokens: Vec<&str> = command_line.split_whitespace().collect();
    let start_idx = if is_interpreter { 1 } else { 0 };

    for token in tokens.iter().skip(start_idx) {
        let cleaned = token.trim_matches('"').trim_matches('\'');
        if cleaned.starts_with('-') {
            continue;
        }
        if cleaned.contains('/') && !cleaned.starts_with("/dev/") {
            return Some(cleaned.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_network_name_ipv4() {
        let binding = parse_network_name("127.0.0.1:8090", "TCP").unwrap();
        assert_eq!(binding.address, "127.0.0.1");
        assert_eq!(binding.port, 8090);
        assert_eq!(binding.protocol, "TCP");
    }

    #[test]
    fn parse_network_name_wildcard() {
        let binding = parse_network_name("*:8090", "TCP").unwrap();
        assert_eq!(binding.address, "*");
        assert_eq!(binding.port, 8090);
    }

    #[test]
    fn extract_script_from_python_command() {
        let cmd = "Python /Users/ginpachi/proj/.server/hosted_web_server.py";
        let path = extract_script_path(cmd, "Python");
        assert_eq!(
            path,
            Some("/Users/ginpachi/proj/.server/hosted_web_server.py".to_string())
        );
    }

    #[test]
    fn scan_listening_ports_live() {
        let processes = scan_listening_ports(false).expect("scan should succeed on macOS");
        assert!(!processes.is_empty(), "expected at least one listener");

        if let Some(python) = processes.iter().find(|p| p.ports.iter().any(|b| b.port == 8090)) {
            assert_eq!(python.name, "Python");
            assert!(!python.working_directory.is_empty() || python.script_path.is_some());
        }
    }
}
