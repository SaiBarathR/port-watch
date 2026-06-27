use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::classifier::{classify as classify_process, SystemKind};
use crate::home::infer_project_root;
use crate::platform::shared::{extract_script_path, parse_address_port};
use crate::scanner::{PortBinding, PortProcess};

#[derive(Debug, Default, Clone)]
struct SocketRecord {
    pid: u32,
    name: String,
    bindings: Vec<PortBinding>,
}

pub fn scan_listening_ports(include_udp: bool) -> Result<Vec<PortProcess>, String> {
    let mut records = run_ss_tcp()?;
    if include_udp {
        records.extend(run_ss_udp()?);
    }

    let records = merge_by_pid(records);
    let mut processes = Vec::new();

    for record in records {
        if record.bindings.is_empty() {
            continue;
        }

        let proc_info = read_proc_info(record.pid)?;
        let script_path = extract_script_path(&proc_info.command_line, &record.name);
        let project_root = infer_project_root(
            if !proc_info.working_directory.is_empty() {
                &proc_info.working_directory
            } else {
                script_path
                    .as_deref()
                    .unwrap_or(&proc_info.executable_path)
            },
        );

        let mut process = PortProcess {
            pid: record.pid,
            name: record.name,
            user: proc_info.user,
            ports: record.bindings,
            executable_path: proc_info.executable_path,
            script_path,
            command_line: proc_info.command_line,
            working_directory: proc_info.working_directory,
            project_root,
            system_kind: SystemKind::User,
            is_system_service: false,
            uptime_seconds: proc_info.uptime_seconds,
        };

        classify_process(&mut process);
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

#[derive(Default)]
struct ProcInfo {
    user: String,
    command_line: String,
    working_directory: String,
    executable_path: String,
    uptime_seconds: u64,
}

fn run_ss_tcp() -> Result<Vec<SocketRecord>, String> {
    run_ss(&["-H", "-tlnp"], "TCP")
}

fn run_ss_udp() -> Result<Vec<SocketRecord>, String> {
    run_ss(&["-H", "-ulnp"], "UDP")
}

fn run_ss(args: &[&str], protocol: &str) -> Result<Vec<SocketRecord>, String> {
    let output = std::process::Command::new("ss")
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run ss: {e}"))?;

    if !output.status.success() && output.stdout.is_empty() {
        return Err(format!(
            "ss exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_ss_output(&stdout, protocol))
}

fn parse_ss_output(stdout: &str, protocol: &str) -> Vec<SocketRecord> {
    let mut records = Vec::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let Some((pid, name, local)) = parse_ss_line(line) else {
            continue;
        };

        let Some(binding) = parse_address_port(local, protocol) else {
            continue;
        };

        records.push(SocketRecord {
            pid,
            name,
            bindings: vec![binding],
        });
    }

    records
}

fn parse_ss_line(line: &str) -> Option<(u32, String, &str)> {
    let users_idx = line.find("users:")?;
    let before_users = line[..users_idx].trim();
    let users_part = &line[users_idx..];

    let pid = users_part
        .split("pid=")
        .nth(1)?
        .split([',', ')'])
        .next()?
        .parse()
        .ok()?;

    let name = users_part
        .split('"')
        .nth(1)
        .unwrap_or("unknown")
        .to_string();

    let parts: Vec<&str> = before_users.split_whitespace().collect();
    let local = *parts.get(parts.len().checked_sub(2)?)?;
    Some((pid, name, local))
}

fn merge_by_pid(records: Vec<SocketRecord>) -> Vec<SocketRecord> {
    let mut by_pid: HashMap<u32, SocketRecord> = HashMap::new();

    for record in records {
        by_pid
            .entry(record.pid)
            .and_modify(|existing| {
                if existing.name.is_empty() {
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

fn read_proc_info(pid: u32) -> Result<ProcInfo, String> {
    let proc_dir = PathBuf::from(format!("/proc/{pid}"));
    if !proc_dir.exists() {
        return Ok(ProcInfo::default());
    }

    let command_line = read_proc_cmdline(&proc_dir);
    let working_directory = fs::read_link(proc_dir.join("cwd"))
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let executable_path = fs::read_link(proc_dir.join("exe"))
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let user = read_proc_user(&proc_dir);
    let uptime_seconds = read_proc_uptime(&proc_dir);

    Ok(ProcInfo {
        user,
        command_line,
        working_directory,
        executable_path,
        uptime_seconds,
    })
}

fn read_proc_cmdline(proc_dir: &PathBuf) -> String {
    fs::read_to_string(proc_dir.join("cmdline"))
        .map(|raw| raw.replace('\0', " ").trim().to_string())
        .unwrap_or_default()
}

fn read_proc_user(proc_dir: &PathBuf) -> String {
    let status = fs::read_to_string(proc_dir.join("status")).unwrap_or_default();
    for line in status.lines() {
        if let Some(uid) = line.strip_prefix("Uid:") {
            let uid = uid.split_whitespace().next().unwrap_or("").trim();
            if let Ok(uid_num) = uid.parse::<u32>() {
                return resolve_uid(uid_num).unwrap_or_else(|| uid.to_string());
            }
        }
    }
    String::new()
}

fn resolve_uid(uid: u32) -> Option<String> {
    let passwd = fs::read_to_string("/etc/passwd").ok()?;
    for line in passwd.lines() {
        let mut parts = line.split(':');
        let name = parts.next()?;
        let _ = parts.next()?;
        let file_uid = parts.next()?.parse::<u32>().ok()?;
        if file_uid == uid {
            return Some(name.to_string());
        }
    }
    None
}

fn read_proc_uptime(proc_dir: &PathBuf) -> u64 {
    use std::sync::OnceLock;

    static CLOCK_TICKS: OnceLock<f64> = OnceLock::new();
    let clock_ticks = *CLOCK_TICKS.get_or_init(|| {
        std::process::Command::new("getconf")
            .arg("CLK_TCK")
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(100.0)
    });

    let stat = fs::read_to_string(proc_dir.join("stat")).unwrap_or_default();
    let start_ticks = stat
        .split_whitespace()
        .nth(21)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let system_uptime = fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|raw| raw.split_whitespace().next()?.parse::<f64>().ok())
        .unwrap_or(0.0);
    let start_secs = start_ticks as f64 / clock_ticks;
    (system_uptime - start_secs).max(0.0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ss_line_extracts_pid_and_local() {
        let line = "0 4096 127.0.0.1:8080 0.0.0.0:* users:((\"node\",pid=1234,fd=21))";
        let (pid, name, local) = parse_ss_line(line).unwrap();
        assert_eq!(pid, 1234);
        assert_eq!(name, "node");
        assert_eq!(local, "127.0.0.1:8080");
    }

    #[test]
    fn parse_ss_line_with_listen_state() {
        let line = "LISTEN 0 4096 127.0.0.1:8080 0.0.0.0:* users:((\"node\",pid=1234,fd=21))";
        let (pid, name, local) = parse_ss_line(line).unwrap();
        assert_eq!(pid, 1234);
        assert_eq!(name, "node");
        assert_eq!(local, "127.0.0.1:8080");
    }

    #[test]
    fn parse_ss_line_ipv6_local() {
        let line = "0 4096 [::1]:3000 0.0.0.0:* users:((\"node\",pid=5678,fd=3))";
        let (pid, name, local) = parse_ss_line(line).unwrap();
        assert_eq!(pid, 5678);
        assert_eq!(name, "node");
        assert_eq!(local, "[::1]:3000");
    }

    #[test]
    fn parse_ss_line_wildcard_local() {
        let line = "0 4096 0.0.0.0:8080 0.0.0.0:* users:((\"nginx\",pid=999,fd=5))";
        let (pid, name, local) = parse_ss_line(line).unwrap();
        assert_eq!(pid, 999);
        assert_eq!(name, "nginx");
        assert_eq!(local, "0.0.0.0:8080");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn scan_listening_ports_live() {
        scan_listening_ports(false).expect("scan should succeed on Linux");
    }
}
