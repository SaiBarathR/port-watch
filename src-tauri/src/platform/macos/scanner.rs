use std::collections::HashMap;
use std::process::Command;

use crate::classifier::{classify as classify_process, SystemKind};
use crate::home::infer_project_root;
use crate::platform::shared::{extract_script_path, run_with_timeout, SCAN_COMMAND_TIMEOUT};
use crate::scanner::{PortBinding, PortProcess};

#[derive(Debug, Default, Clone)]
struct PsInfo {
    user: String,
    command_line: String,
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
        let project_root = infer_project_root(if !path_info.working_directory.is_empty() {
            &path_info.working_directory
        } else {
            script_path.as_deref().unwrap_or(&path_info.executable_path)
        });

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
            started_at: super::shell::process_started_at(pid).unwrap_or(0),
            delete_blocked: None,
        };

        classify_process(&mut process);
        processes.push(process);
    }

    Ok(processes)
}

fn run_lsof_tcp() -> Result<Vec<LsofRecord>, String> {
    let output = run_with_timeout(
        Command::new("lsof").args(["-iTCP", "-sTCP:LISTEN", "-n", "-P", "-F", "pcn"]),
        SCAN_COMMAND_TIMEOUT,
    )
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
    let output = run_with_timeout(
        Command::new("lsof").args(["-iUDP", "-n", "-P", "-F", "pcn"]),
        SCAN_COMMAND_TIMEOUT,
    )
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
                current.name = Some(unescape_lsof(value));
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

// lsof escapes a backslash as `\\` and, when the app runs without a locale
// (launched from Finder or the Dock), every non-ASCII byte as `\xNN`. Undoing
// both yields the kernel's own name for the process, which is what the stop
// path checks a PID against.
fn unescape_lsof(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut unescaped = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'\\' {
            match bytes.get(index + 1) {
                Some(b'\\') => {
                    unescaped.push(b'\\');
                    index += 2;
                    continue;
                }
                Some(b'x') => {
                    let byte = bytes
                        .get(index + 2..index + 4)
                        .filter(|hex| hex.iter().all(u8::is_ascii_hexdigit))
                        .and_then(|hex| std::str::from_utf8(hex).ok())
                        .and_then(|hex| u8::from_str_radix(hex, 16).ok());
                    if let Some(byte) = byte {
                        unescaped.push(byte);
                        index += 4;
                        continue;
                    }
                }
                _ => {}
            }
        }
        unescaped.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&unescaped).into_owned()
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

        let output = run_with_timeout(
            Command::new("ps").args(["-ww", "-p", &pid_list, "-o", "pid=,user=,command="]),
            SCAN_COMMAND_TIMEOUT,
        )
        .map_err(|e| format!("Failed to run ps: {e}"))?;

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some((pid, info)) = parse_ps_line(line) {
                result.insert(pid, info);
            }
        }
    }

    Ok(result)
}

// ps pads columns with runs of spaces, so grab the first two tokens and
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

    Some((
        pid,
        PsInfo {
            user: user.to_string(),
            command_line: rest.trim().to_string(),
        },
    ))
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

        let output = match run_with_timeout(
            Command::new("lsof").args(["-a", "-p", &pid_list, "-d", "cwd,txt", "-Fn"]),
            SCAN_COMMAND_TIMEOUT,
        ) {
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
    fn unescape_lsof_restores_the_kernel_name() {
        assert_eq!(unescape_lsof("node"), "node");
        assert_eq!(
            unescape_lsof("Caf\\xc3\\xa9 S\\xc3\\xabrver"),
            "Café Sërver"
        );
        assert_eq!(unescape_lsof("back\\\\slash name"), "back\\slash name");
        // An escaped backslash followed by `x41` is not the byte 0x41.
        assert_eq!(unescape_lsof("a\\\\x41"), "a\\x41");
        // Not escapes lsof produces: left as they are.
        assert_eq!(unescape_lsof("a\\xzz"), "a\\xzz");
        assert_eq!(unescape_lsof("trailing\\"), "trailing\\");
    }

    #[test]
    fn parse_lsof_output_unescapes_command_names() {
        let records = parse_lsof_output("p42\ncCaf\\xc3\\xa9\nn127.0.0.1:8080\n", "TCP");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].name.as_deref(), Some("Café"));
    }

    #[test]
    fn parse_ps_line_with_padded_columns() {
        let (pid, info) =
            parse_ps_line("  501 root      node server.js --port 3000").expect("line should parse");
        assert_eq!(pid, 501);
        assert_eq!(info.user, "root");
        assert_eq!(info.command_line, "node server.js --port 3000");
    }

    #[test]
    fn parse_ps_line_rejects_garbage() {
        assert!(parse_ps_line("ps: illegal process id: x").is_none());
        assert!(parse_ps_line("").is_none());
    }
}
