use std::collections::HashMap;

use crate::classifier::{classify as classify_process, SystemKind};
use crate::home::infer_project_root;
use crate::platform::shared::extract_script_path;
use crate::scanner::{PortBinding, PortProcess};

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

fn run_lsof_tcp() -> Result<Vec<LsofRecord>, String> {
    let output = std::process::Command::new("lsof")
        .args(["-iTCP", "-sTCP:LISTEN", "-n", "-P", "-F", "pcn"])
        .output()
        .map_err(|e| format!("Failed to run lsof: {e}"))?;

    if !output.status.success() && output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.trim().is_empty() {
            return Ok(Vec::new());
        }
        return Err(format!(
            "lsof exited with status {}: {stderr}",
            output.status
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
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.trim().is_empty() {
            return Ok(Vec::new());
        }
        return Err(format!(
            "lsof UDP exited with status {}: {stderr}",
            output.status
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
                if let Some(binding) = crate::platform::shared::parse_address_port(value, protocol)
                {
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

        // BSD ps has no `etimes` keyword — only `etime` ([[dd-]hh:]mm:ss).
        let output = std::process::Command::new("ps")
            .args(["-ww", "-p", &pid_list, "-o", "pid=,user=,etime=,command="])
            .output()
            .map_err(|e| format!("Failed to run ps: {e}"))?;

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some((pid, info)) = parse_ps_line(line) {
                result.insert(pid, info);
            }
        }
    }

    Ok(result)
}

// ps pads columns with runs of spaces, so grab the first three tokens and
// keep the remainder verbatim as the command line.
fn split_token(input: &str) -> (&str, &str) {
    let trimmed = input.trim_start();
    match trimmed.find(char::is_whitespace) {
        Some(idx) => (&trimmed[..idx], &trimmed[idx..]),
        None => (trimmed, ""),
    }
}

fn parse_ps_line(line: &str) -> Option<(u32, PsInfo)> {
    let (pid_str, rest) = split_token(line);
    let pid = pid_str.parse::<u32>().ok()?;
    let (user, rest) = split_token(rest);
    let (etime, rest) = split_token(rest);

    Some((
        pid,
        PsInfo {
            user: user.to_string(),
            command_line: rest.trim().to_string(),
            uptime_seconds: parse_etime(etime),
        },
    ))
}

fn parse_etime(value: &str) -> u64 {
    let (days, clock) = match value.split_once('-') {
        Some((days, clock)) => (days.parse::<u64>().unwrap_or(0), clock),
        None => (0, value),
    };

    let mut parts = clock.split(':').rev();
    let seconds = parts.next().and_then(|p| p.parse::<u64>().ok()).unwrap_or(0);
    let minutes = parts.next().and_then(|p| p.parse::<u64>().ok()).unwrap_or(0);
    let hours = parts.next().and_then(|p| p.parse::<u64>().ok()).unwrap_or(0);

    days * 86_400 + hours * 3_600 + minutes * 60 + seconds
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
                        // lsof lists the executable as the first txt entry,
                        // followed by mapped dylibs/caches — keep only the first.
                        Some("txt") if entry.executable_path.is_empty() => {
                            entry.executable_path = value.to_string();
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_listening_ports_live() {
        scan_listening_ports(false).expect("scan should succeed on macOS");
    }

    #[test]
    fn parse_etime_formats() {
        assert_eq!(parse_etime("05"), 5);
        assert_eq!(parse_etime("04:05"), 245);
        assert_eq!(parse_etime("03:04:05"), 11_045);
        assert_eq!(parse_etime("2-03:04:05"), 183_845);
        assert_eq!(parse_etime(""), 0);
    }

    #[test]
    fn parse_ps_line_with_padded_columns() {
        let (pid, info) =
            parse_ps_line("  501 root      1-02:03:04 node server.js --port 3000")
                .expect("line should parse");
        assert_eq!(pid, 501);
        assert_eq!(info.user, "root");
        assert_eq!(info.uptime_seconds, 93_784);
        assert_eq!(info.command_line, "node server.js --port 3000");
    }

    #[test]
    fn parse_ps_line_rejects_garbage() {
        assert!(parse_ps_line("ps: etimes: keyword not found").is_none());
        assert!(parse_ps_line("").is_none());
    }
}
