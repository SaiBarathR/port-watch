use std::path::Path;

use serde::Serialize;

use crate::platform;
use crate::platform::path_validation::DeleteRules;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PortBinding {
    pub address: String,
    pub port: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
    pub system_kind: crate::classifier::SystemKind,
    pub is_system_service: bool,
    /// When the process started, in Unix seconds; 0 when that is unknown.
    /// A start time rather than an uptime, so a process that has not changed
    /// compares equal from one scan to the next.
    pub started_at: u64,
    /// Why the app will not delete this process's project folder, if it will
    /// not. The UI disables the action and shows the reason.
    pub delete_blocked: Option<String>,
}

/// What tells a process from a later one that was handed the same PID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub name: String,
    /// Unix seconds; 0 when the scan could not tell.
    pub started_at: u64,
}

impl PortProcess {
    pub fn identity(&self) -> ProcessIdentity {
        ProcessIdentity {
            name: self.name.clone(),
            started_at: self.started_at,
        }
    }

    /// The folder the reveal, terminal, editor and delete actions work on:
    /// the project root when one was found, else the working directory.
    pub fn project_dir(&self) -> &str {
        if self.project_root.is_empty() {
            &self.working_directory
        } else {
            &self.project_root
        }
    }
}

pub fn scan_listening_ports(include_udp: bool) -> Result<Vec<PortProcess>, String> {
    let mut processes = platform::scan_listening_ports(include_udp)?;
    sort_processes(&mut processes);
    mark_undeletable_folders(&mut processes, DeleteRules::for_current_user());
    Ok(processes)
}

// By first port, as the table shows them. The platform scanners collect
// processes in hash maps, so ties must be broken the same way every time:
// two scans of an unchanged machine have to compare equal, or every scan
// would be announced as a change.
fn sort_processes(processes: &mut [PortProcess]) {
    fn key(process: &PortProcess) -> (u16, u32, &str, &str) {
        let first = process.ports.first();
        (
            first.map(|binding| binding.port).unwrap_or(0),
            process.pid,
            first.map(|binding| binding.address.as_str()).unwrap_or(""),
            first.map(|binding| binding.protocol.as_str()).unwrap_or(""),
        )
    }
    processes.sort_by(|a, b| key(a).cmp(&key(b)));
}

// A platform scanner may already have blocked a folder for a reason only it
// knows; that is kept.
fn mark_undeletable_folders(processes: &mut [PortProcess], rules: Result<DeleteRules, String>) {
    for process in processes {
        if process.delete_blocked.is_some() {
            continue;
        }
        process.delete_blocked = match &rules {
            Ok(rules) => rules.resolve(Path::new(process.project_dir())).err(),
            Err(err) => Some(err.clone()),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classifier::SystemKind;
    use crate::platform::shared::parse_address_port;

    fn listener(pid: u32, address: &str, port: u16) -> PortProcess {
        PortProcess {
            pid,
            name: "nginx".into(),
            user: "dev".into(),
            ports: vec![PortBinding {
                address: address.into(),
                port,
                protocol: "TCP".into(),
            }],
            executable_path: String::new(),
            script_path: None,
            command_line: String::new(),
            working_directory: String::new(),
            project_root: String::new(),
            system_kind: SystemKind::User,
            is_system_service: false,
            started_at: 0,
            delete_blocked: None,
        }
    }

    // A master and its workers share a port, and on Linux several sockets
    // nobody can be named for share PID 0. Whatever order they are found in,
    // they come out the same.
    #[test]
    fn processes_sharing_a_first_port_always_sort_the_same_way() {
        let sorted = |mut processes: Vec<PortProcess>| {
            sort_processes(&mut processes);
            processes
                .iter()
                .map(|process| {
                    (
                        process.ports[0].port,
                        process.pid,
                        process.ports[0].address.clone(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let one_order = vec![
            listener(300, "*", 8080),
            listener(0, "[::]", 22),
            listener(200, "*", 80),
            listener(0, "0.0.0.0", 22),
            listener(100, "*", 80),
        ];
        let mut another_order = one_order.clone();
        another_order.reverse();

        let expected = vec![
            (22, 0, "0.0.0.0".to_string()),
            (22, 0, "[::]".to_string()),
            (80, 100, "*".to_string()),
            (80, 200, "*".to_string()),
            (8080, 300, "*".to_string()),
        ];
        assert_eq!(sorted(one_order), expected);
        assert_eq!(sorted(another_order), expected);
    }

    #[test]
    fn parse_network_name_ipv4() {
        let binding = parse_address_port("127.0.0.1:8090", "TCP").unwrap();
        assert_eq!(binding.address, "127.0.0.1");
        assert_eq!(binding.port, 8090);
        assert_eq!(binding.protocol, "TCP");
    }

    #[test]
    fn parse_network_name_wildcard() {
        let binding = parse_address_port("*:8090", "TCP").unwrap();
        assert_eq!(binding.address, "*");
        assert_eq!(binding.port, 8090);
    }

    #[test]
    fn extract_script_from_python_command() {
        let cmd = "Python /Users/ginpachi/proj/.server/hosted_web_server.py";
        let path = crate::platform::shared::extract_script_path(cmd, "Python");
        assert_eq!(
            path,
            Some("/Users/ginpachi/proj/.server/hosted_web_server.py".to_string())
        );
    }
}
