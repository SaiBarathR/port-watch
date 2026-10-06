use std::process;

use crate::scanner::{scan_listening_ports, PortProcess};

pub fn run_check(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: port-watch check <port> [--udp]");
        process::exit(2);
    }

    let port: u16 = match args[0].parse() {
        Ok(port) => port,
        Err(_) => {
            eprintln!("Invalid port: {}", args[0]);
            process::exit(2);
        }
    };

    let include_udp = args.get(1).map(|v| v == "--udp").unwrap_or(false);
    let processes = match scan_listening_ports(include_udp) {
        Ok(processes) => processes,
        Err(err) => {
            eprintln!("{err}");
            process::exit(2);
        }
    };

    let owners: Vec<&PortProcess> = processes
        .iter()
        .filter(|process| process.ports.iter().any(|binding| binding.port == port))
        .collect();

    if owners.is_empty() {
        process::exit(0);
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let owners: Vec<Owner> = owners
        .into_iter()
        .map(|process| Owner::at(process, now))
        .collect();

    println!(
        "{}",
        serde_json::to_string(&owners).unwrap_or_else(|_| "[]".into())
    );
    process::exit(1);
}

/// A process as `check` prints it: the scan's fields, plus the uptime the
/// output had before the scan switched to a start time.
#[derive(serde::Serialize)]
struct Owner<'a> {
    #[serde(flatten)]
    process: &'a PortProcess,
    uptime_seconds: u64,
}

impl<'a> Owner<'a> {
    fn at(process: &'a PortProcess, now: u64) -> Self {
        let uptime_seconds = match process.started_at {
            0 => 0,
            started_at => now.saturating_sub(started_at),
        };
        Self {
            process,
            uptime_seconds,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classifier::SystemKind;
    use crate::scanner::PortBinding;

    fn process(started_at: u64) -> PortProcess {
        PortProcess {
            id: String::new(),
            pid: 42,
            name: "node".into(),
            user: "dev".into(),
            ports: vec![PortBinding {
                address: "*".into(),
                port: 3000,
                protocol: "TCP".into(),
            }],
            executable_path: "/usr/local/bin/node".into(),
            script_path: None,
            command_line: "node server.js".into(),
            working_directory: "/Users/dev/app".into(),
            project_root: "/Users/dev/app".into(),
            system_kind: SystemKind::User,
            is_system_service: false,
            started_at,
            delete_blocked: None,
        }
    }

    #[test]
    fn output_keeps_uptime_seconds_next_to_the_scan_fields() {
        let process = process(1_000);
        let json = serde_json::to_value(Owner::at(&process, 1_090)).unwrap();
        assert_eq!(json["pid"], 42);
        assert_eq!(json["started_at"], 1_000);
        assert_eq!(json["uptime_seconds"], 90);
        assert_eq!(json["ports"][0]["port"], 3000);
    }

    #[test]
    fn uptime_is_zero_when_the_start_time_is_unknown_or_in_the_future() {
        assert_eq!(Owner::at(&process(0), 1_090).uptime_seconds, 0);
        assert_eq!(Owner::at(&process(2_000), 1_090).uptime_seconds, 0);
    }
}
