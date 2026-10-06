use std::collections::HashMap;
use std::process::Command;

use crate::platform::parsers::{lsof, ps};
use crate::platform::shared::{run_with_timeout, SCAN_COMMAND_TIMEOUT};
use crate::scanner::{ProcessDetails, RawScan, RawSocket};

const PS_BATCH_SIZE: usize = 100;
const LSOF_BATCH_SIZE: usize = 50;

pub fn scan(include_udp: bool) -> Result<RawScan, String> {
    let mut sockets = list_sockets(&["-iTCP", "-sTCP:LISTEN"], "TCP")?;
    if include_udp {
        sockets.extend(list_sockets(&["-iUDP"], "UDP")?);
    }

    let mut pids: Vec<u32> = sockets.iter().map(|socket| socket.pid).collect();
    pids.sort_unstable();
    pids.dedup();

    // Asked of the kernel, which costs nothing next to starting a program.
    // What it will not say goes to the tools a scan used to run for all of
    // them.
    let mut details = HashMap::new();
    let mut unread = Vec::new();
    for pid in pids {
        match super::process::read_details(pid) {
            Some(read) => {
                details.insert(pid, read);
            }
            None => unread.push(pid),
        }
    }
    details.extend(read_details_with_tools(&unread)?);

    Ok(RawScan { sockets, details })
}

fn read_details_with_tools(pids: &[u32]) -> Result<HashMap<u32, ProcessDetails>, String> {
    let mut ps_info = fetch_ps_info(pids)?;
    let mut paths = fetch_paths(pids);

    Ok(pids
        .iter()
        .map(|&pid| {
            let ps = ps_info.remove(&pid).unwrap_or_default();
            let paths = paths.remove(&pid).unwrap_or_default();
            let details = ProcessDetails {
                user: ps.user,
                command_line: ps.command_line,
                working_directory: paths.working_directory,
                executable_path: paths.executable_path,
                started_at: super::shell::process_started_at(pid).unwrap_or(0),
                delete_blocked: None,
            };
            (pid, details)
        })
        .collect())
}

fn list_sockets(selection: &[&str], protocol: &str) -> Result<Vec<RawSocket>, String> {
    let output = run_with_timeout(
        Command::new("lsof")
            .args(selection)
            .args(["-n", "-P", "-F", "pcn"]),
        SCAN_COMMAND_TIMEOUT,
    )
    .map_err(|e| format!("Failed to run lsof for {protocol}: {e}"))?;

    // lsof exits with an error when it has nothing to list.
    if !output.status.success() && output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.trim().is_empty() {
            return Ok(Vec::new());
        }
        return Err(format!(
            "lsof for {protocol} exited with status {}: {stderr}",
            output.status
        ));
    }

    Ok(lsof::parse_listeners(
        &String::from_utf8_lossy(&output.stdout),
        protocol,
    ))
}

fn pid_list(pids: &[u32]) -> String {
    pids.iter()
        .map(|pid| pid.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn fetch_ps_info(pids: &[u32]) -> Result<HashMap<u32, ps::PsInfo>, String> {
    let mut info = HashMap::new();
    for chunk in pids.chunks(PS_BATCH_SIZE) {
        let output = run_with_timeout(
            Command::new("ps").args(["-ww", "-p", &pid_list(chunk), "-o", "pid=,user=,command="]),
            SCAN_COMMAND_TIMEOUT,
        )
        .map_err(|e| format!("Failed to run ps: {e}"))?;
        info.extend(ps::parse(&String::from_utf8_lossy(&output.stdout)));
    }
    Ok(info)
}

// Best effort: a process whose paths cannot be read is still listed.
fn fetch_paths(pids: &[u32]) -> HashMap<u32, lsof::ProcessPaths> {
    let mut paths = HashMap::new();
    for chunk in pids.chunks(LSOF_BATCH_SIZE) {
        if let Ok(output) = run_with_timeout(
            Command::new("lsof").args(["-a", "-p", &pid_list(chunk), "-d", "cwd,txt", "-Fn"]),
            SCAN_COMMAND_TIMEOUT,
        ) {
            paths.extend(lsof::parse_paths(&String::from_utf8_lossy(&output.stdout)));
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_live() {
        scan(false).expect("scan should succeed on macOS");
    }

    // The path a scan took for every process before, and still takes for
    // one the kernel will not describe.
    #[test]
    fn the_tools_describe_a_process_too() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().canonicalize().unwrap();
        let mut child = crate::platform::unix::testing::spawn_sleep(Some(&folder));
        let pid = child.id();
        let details = read_details_with_tools(&[pid]);
        let _ = child.kill();
        let _ = child.wait();

        let details = details.unwrap().remove(&pid).unwrap();
        assert_eq!(details.command_line, "sleep 60");
        assert_eq!(details.working_directory, folder.to_string_lossy());
        assert!(details.executable_path.ends_with("/sleep"));
        assert!(!details.user.is_empty());
        assert!(details.started_at > 0);
    }
}
