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

        let Some((owners, local)) = parse_ss_line(line) else {
            continue;
        };

        let Some(binding) = parse_address_port(local, protocol) else {
            continue;
        };

        // Pre-forked servers (nginx, gunicorn) share one listening socket
        // across master and workers; ss lists every owner in one line, and
        // each deserves its own record so stopping shows all involved pids.
        for (pid, name) in owners {
            records.push(SocketRecord {
                pid,
                name,
                bindings: vec![binding.clone()],
            });
        }
    }

    records
}

// Every ("name",pid=N,fd=M) owner in a users:(...) field.
fn parse_ss_owners(users_part: &str) -> Vec<(u32, String)> {
    let mut owners = Vec::new();

    for segment in users_part.split("(\"").skip(1) {
        let name = segment.split('"').next().unwrap_or("unknown").to_string();
        let pid = segment
            .split("pid=")
            .nth(1)
            .and_then(|rest| rest.split([',', ')']).next())
            .and_then(|value| value.parse().ok());
        if let Some(pid) = pid {
            owners.push((pid, name));
        }
    }

    owners
}

fn parse_ss_line(line: &str) -> Option<(Vec<(u32, String)>, &str)> {
    // `ss` only attaches the `users:(...)` process field for sockets the caller
    // owns (or all of them when running as root). Sockets owned by other users
    // appear without it, so the field is optional: keep the listener with an
    // unknown owner (pid 0) rather than dropping it from the scan.
    let (before_users, users_part) = match line.find("users:") {
        Some(idx) => (line[..idx].trim(), Some(&line[idx..])),
        None => (line.trim(), None),
    };

    let owners = match users_part {
        Some(users_part) => {
            let owners = parse_ss_owners(users_part);
            if owners.is_empty() {
                return None;
            }
            owners
        }
        None => vec![(0, "unknown".to_string())],
    };

    let parts: Vec<&str> = before_users.split_whitespace().collect();
    let local = *parts.get(parts.len().checked_sub(2)?)?;
    Some((owners, local))
}

fn merge_by_pid(records: Vec<SocketRecord>) -> Vec<SocketRecord> {
    let mut by_pid: HashMap<u32, SocketRecord> = HashMap::new();
    // Ownerless sockets (pid 0, e.g. other users' listeners seen without root)
    // share the placeholder pid, so they must not be merged into one another.
    let mut ownerless: Vec<SocketRecord> = Vec::new();

    for record in records {
        if record.pid == 0 {
            ownerless.push(record);
            continue;
        }

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

    let mut merged: Vec<SocketRecord> = by_pid.into_values().collect();
    merged.extend(ownerless);
    merged
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
    use std::collections::HashMap;
    use std::sync::Mutex;

    static CACHE: Mutex<Option<HashMap<u32, Option<String>>>> = Mutex::new(None);

    if let Ok(mut guard) = CACHE.lock() {
        if let Some(cached) = guard.get_or_insert_with(HashMap::new).get(&uid) {
            return cached.clone();
        }
    }

    let resolved = resolve_uid_uncached(uid);

    if let Ok(mut guard) = CACHE.lock() {
        guard
            .get_or_insert_with(HashMap::new)
            .insert(uid, resolved.clone());
    }

    resolved
}

fn resolve_uid_uncached(uid: u32) -> Option<String> {
    if let Ok(passwd) = fs::read_to_string("/etc/passwd") {
        for line in passwd.lines() {
            let mut parts = line.split(':');
            let Some(name) = parts.next() else { continue };
            let Some(_) = parts.next() else { continue };
            let Some(file_uid) = parts.next().and_then(|v| v.parse::<u32>().ok()) else {
                continue;
            };
            if file_uid == uid {
                return Some(name.to_string());
            }
        }
    }

    // NSS-managed users (LDAP, SSSD, systemd-homed) aren't in /etc/passwd.
    let output = std::process::Command::new("getent")
        .args(["passwd", &uid.to_string()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let name = stdout.split(':').next()?.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
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
    // The comm field (2nd) can contain spaces and parens — e.g. "(tmux: server)"
    // — so split after its closing paren; starttime is overall field 22, i.e.
    // the 20th field after state.
    let start_ticks = stat
        .rsplit_once(')')
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_whitespace().nth(19))
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
        let (owners, local) = parse_ss_line(line).unwrap();
        assert_eq!(owners, vec![(1234, "node".to_string())]);
        assert_eq!(local, "127.0.0.1:8080");
    }

    #[test]
    fn parse_ss_line_with_listen_state() {
        let line = "LISTEN 0 4096 127.0.0.1:8080 0.0.0.0:* users:((\"node\",pid=1234,fd=21))";
        let (owners, local) = parse_ss_line(line).unwrap();
        assert_eq!(owners, vec![(1234, "node".to_string())]);
        assert_eq!(local, "127.0.0.1:8080");
    }

    #[test]
    fn parse_ss_line_ipv6_local() {
        let line = "0 4096 [::1]:3000 0.0.0.0:* users:((\"node\",pid=5678,fd=3))";
        let (owners, local) = parse_ss_line(line).unwrap();
        assert_eq!(owners, vec![(5678, "node".to_string())]);
        assert_eq!(local, "[::1]:3000");
    }

    #[test]
    fn parse_ss_line_wildcard_local() {
        let line = "0 4096 0.0.0.0:8080 0.0.0.0:* users:((\"nginx\",pid=999,fd=5))";
        let (owners, local) = parse_ss_line(line).unwrap();
        assert_eq!(owners, vec![(999, "nginx".to_string())]);
        assert_eq!(local, "0.0.0.0:8080");
    }

    #[test]
    fn parse_ss_line_multiple_owners() {
        // Pre-forked servers share one listening socket across master and
        // workers; every owner must be reported.
        let line = "LISTEN 0 511 0.0.0.0:80 0.0.0.0:* users:((\"nginx\",pid=101,fd=6),(\"nginx\",pid=100,fd=6))";
        let (owners, local) = parse_ss_line(line).unwrap();
        assert_eq!(
            owners,
            vec![(101, "nginx".to_string()), (100, "nginx".to_string())]
        );
        assert_eq!(local, "0.0.0.0:80");
    }

    #[test]
    fn parse_ss_line_without_users_field() {
        // Non-root `ss` omits the users:(...) field for sockets owned by other
        // users; the listener must still be parsed with an unknown owner.
        let line = "LISTEN 0 4096 0.0.0.0:443 0.0.0.0:*";
        let (owners, local) = parse_ss_line(line).unwrap();
        assert_eq!(owners, vec![(0, "unknown".to_string())]);
        assert_eq!(local, "0.0.0.0:443");
    }

    #[test]
    fn merge_by_pid_keeps_ownerless_sockets_separate() {
        let records = vec![
            SocketRecord {
                pid: 0,
                name: "unknown".into(),
                bindings: vec![PortBinding {
                    address: "0.0.0.0".into(),
                    port: 443,
                    protocol: "TCP".into(),
                }],
            },
            SocketRecord {
                pid: 0,
                name: "unknown".into(),
                bindings: vec![PortBinding {
                    address: "0.0.0.0".into(),
                    port: 80,
                    protocol: "TCP".into(),
                }],
            },
        ];
        let merged = merge_by_pid(records);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn scan_listening_ports_live() {
        scan_listening_ports(false).expect("scan should succeed on Linux");
    }
}
