use std::path::Path;

use crate::platform::parsers::powershell::{self, Listener};
use crate::platform::shared::{extract_script_path, run_with_timeout};
use crate::scanner::{PortBinding, ProcessDetails, RawScan, RawSocket};

// PowerShell alone can take seconds to start on a cold or busy machine, so it
// gets far longer than the Unix tools before it counts as stuck.
const POWERSHELL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub fn scan(include_udp: bool) -> Result<RawScan, String> {
    // Taken before PowerShell starts, so anything that was already running
    // by now is what the snapshot below describes.
    let scan_began = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);

    let mut raw = RawScan::default();
    for listener in query_listeners(include_udp)? {
        raw.sockets.push(RawSocket {
            pid: listener.pid,
            name: listener.name.clone(),
            binding: PortBinding {
                address: powershell::normalize_address(&listener.local_address),
                port: listener.local_port,
                protocol: listener.protocol.clone(),
            },
        });
        raw.details.entry(listener.pid).or_insert_with(|| {
            let kernel_started_at = super::shell::process_started_at(listener.pid);
            details_of(listener, kernel_started_at, scan_began)
        });
    }

    Ok(raw)
}

fn details_of(
    listener: Listener,
    kernel_started_at: Option<u64>,
    scan_began: u64,
) -> ProcessDetails {
    let executable_path = listener.executable_path.unwrap_or_default();
    let command_line = listener.command_line.unwrap_or_default();
    let script_path = extract_script_path(&command_line, &listener.name);

    ProcessDetails {
        user: listener.user,
        // Windows does not say where a process is running, so the folder is
        // taken from its script, or failing that from its executable.
        working_directory: infer_working_directory(&executable_path, &script_path),
        // Without a script, that folder is just where the program is
        // installed (per-user VS Code, Cursor, ...), not a project.
        delete_blocked: script_path.is_none().then(|| {
            "The folder was guessed from where the program is installed, so it may not be a project."
                .to_string()
        }),
        executable_path,
        command_line,
        started_at: started_at(kernel_started_at, listener.started_at, scan_began),
    }
}

// The kernel's own figure, read the way a stop reads it, when it is sure to
// be the snapshot's process: one that was running before the scan began. A
// PID handed to another process after the snapshot was taken shows a start
// time later than that, and must not lend it to the row the old process left
// behind. For that case, and for a process that cannot be read, PowerShell's
// figure stands; it passes through local time, which is ambiguous for an hour
// each year.
fn started_at(kernel: Option<u64>, reported: i64, scan_began: u64) -> u64 {
    match kernel {
        Some(kernel) if kernel < scan_began => kernel,
        _ => reported.max(0) as u64,
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

fn query_listeners(include_udp: bool) -> Result<Vec<Listener>, String> {
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

    powershell::parse(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listener(json: &str) -> Listener {
        powershell::parse(json)
            .unwrap()
            .into_iter()
            .next()
            .expect("one listener")
    }

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
    fn a_process_with_a_script_runs_in_the_scripts_folder() {
        let details = details_of(
            listener(
                r#"{"pid":4242,"name":"node.exe","user":"PC\\dev","localAddress":"::","localPort":3000,"executablePath":"C:\\Program Files\\nodejs\\node.exe","commandLine":"node C:\\Users\\dev\\app\\server.js","protocol":"TCP","startedAt":1790000000}"#,
            ),
            None,
            1_790_000_500,
        );

        assert_eq!(details.user, "PC\\dev");
        assert_eq!(details.working_directory, "C:\\Users\\dev\\app");
        assert_eq!(details.delete_blocked, None);
        assert_eq!(details.started_at, 1_790_000_000);
    }

    #[test]
    fn a_folder_guessed_from_the_executable_is_not_deletable() {
        let details = details_of(
            listener(
                r#"{"pid":4242,"name":"Code.exe","user":"PC\\dev","localAddress":"127.0.0.1","localPort":3000,"executablePath":"C:\\Users\\dev\\AppData\\Local\\Programs\\VS Code\\Code.exe","commandLine":"Code.exe","protocol":"TCP","startedAt":1790000000}"#,
            ),
            Some(1_790_000_001),
            1_790_000_500,
        );

        assert_eq!(
            details.working_directory,
            "C:\\Users\\dev\\AppData\\Local\\Programs\\VS Code"
        );
        assert!(details.delete_blocked.is_some());
        assert_eq!(details.started_at, 1_790_000_001);
    }

    #[test]
    fn the_kernels_start_time_is_used_for_a_process_older_than_the_scan() {
        assert_eq!(started_at(Some(1_000), 4_600, 2_000), 1_000);
    }

    // A PID reused after the snapshot was taken belongs to a process that
    // started once the scan had begun. Its start time is not the row's.
    #[test]
    fn a_start_time_from_after_the_scan_began_is_not_trusted() {
        assert_eq!(started_at(Some(2_000), 1_000, 2_000), 1_000);
        assert_eq!(started_at(Some(2_005), 1_000, 2_000), 1_000);
    }

    #[test]
    fn an_unreadable_process_keeps_the_reported_start_time() {
        assert_eq!(started_at(None, 1_000, 2_000), 1_000);
        assert_eq!(started_at(None, -5, 2_000), 0);
    }

    #[test]
    fn scan_live() {
        scan(false).expect("scan should succeed on Windows");
    }
}
