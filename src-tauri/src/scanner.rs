use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use crate::classifier::SystemKind;
use crate::home::infer_project_root;
use crate::platform;
use crate::platform::path_validation::DeleteRules;
use crate::platform::shared::extract_script_path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PortBinding {
    pub address: String,
    pub port: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PortProcess {
    /// Names this row across scans. The PID cannot: on Linux every listener
    /// whose owner is not visible is reported under PID 0.
    pub id: String,
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

/// One listening socket, and the process a platform says owns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSocket {
    /// 0 when the owner is not visible to this user.
    pub pid: u32,
    pub name: String,
    pub binding: PortBinding,
}

/// What a platform knows about a process besides its sockets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessDetails {
    pub user: String,
    pub command_line: String,
    pub working_directory: String,
    pub executable_path: String,
    /// Unix seconds; 0 when unknown.
    pub started_at: u64,
    /// Set when the platform already knows the folder is not a project.
    pub delete_blocked: Option<String>,
}

/// Everything a platform contributes to a scan. The rest is `assemble`.
#[derive(Debug, Default)]
pub struct RawScan {
    pub sockets: Vec<RawSocket>,
    pub details: HashMap<u32, ProcessDetails>,
}

pub fn scan_listening_ports(include_udp: bool) -> Result<Vec<PortProcess>, String> {
    let mut processes = assemble(platform::scan(include_udp)?, platform::classify);
    mark_undeletable_folders(&mut processes, DeleteRules::for_current_user());
    Ok(processes)
}

// One row per process, holding each of its sockets once. Sockets with no
// visible owner all carry PID 0 and are not one process, so each stays a row
// of its own.
fn assemble(raw: RawScan, classify: impl Fn(&mut PortProcess)) -> Vec<PortProcess> {
    let RawScan { sockets, details } = raw;
    let mut processes: Vec<PortProcess> = Vec::new();
    let mut row_of: HashMap<u32, usize> = HashMap::new();

    for socket in sockets {
        if socket.pid != 0 {
            if let Some(&row) = row_of.get(&socket.pid) {
                let process = &mut processes[row];
                if process.name.is_empty() {
                    process.name = socket.name;
                }
                if !process.ports.contains(&socket.binding) {
                    process.ports.push(socket.binding);
                }
                continue;
            }
            row_of.insert(socket.pid, processes.len());
        }

        let details = details.get(&socket.pid).cloned().unwrap_or_default();
        processes.push(describe(socket, details));
    }

    processes.iter_mut().for_each(classify);
    sort_processes(&mut processes);
    assign_ids(&mut processes);
    processes
}

fn describe(socket: RawSocket, details: ProcessDetails) -> PortProcess {
    let script_path = extract_script_path(&details.command_line, &socket.name);
    let project_root = infer_project_root(if !details.working_directory.is_empty() {
        &details.working_directory
    } else {
        script_path.as_deref().unwrap_or(&details.executable_path)
    });

    PortProcess {
        id: String::new(),
        pid: socket.pid,
        name: socket.name,
        user: details.user,
        ports: vec![socket.binding],
        executable_path: details.executable_path,
        script_path,
        command_line: details.command_line,
        working_directory: details.working_directory,
        project_root,
        system_kind: SystemKind::User,
        is_system_service: false,
        started_at: details.started_at,
        delete_blocked: details.delete_blocked,
    }
}

// `pid-<pid>` for a process, and the socket itself for a listener with no
// visible owner. Run after sorting, so that two identical ownerless sockets
// get the same suffixes on every scan.
fn assign_ids(processes: &mut [PortProcess]) {
    let mut seen = std::collections::HashMap::new();
    for process in processes {
        let base = match (process.pid, process.ports.first()) {
            (0, Some(binding)) => format!(
                "socket-{}-{}-{}",
                binding.protocol.to_ascii_lowercase(),
                binding.address,
                binding.port
            ),
            (pid, _) => format!("pid-{pid}"),
        };
        let count = seen.entry(base.clone()).or_insert(0u32);
        *count += 1;
        process.id = if *count == 1 {
            base
        } else {
            format!("{base}#{count}")
        };
    }
}

// By first port, as the table shows them. Ties must be broken the same way
// every time: two scans of an unchanged machine have to compare equal, or
// every scan would be announced as a change.
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
    use crate::platform::shared::parse_address_port;

    fn socket(pid: u32, name: &str, address: &str, port: u16, protocol: &str) -> RawSocket {
        RawSocket {
            pid,
            name: name.into(),
            binding: PortBinding {
                address: address.into(),
                port,
                protocol: protocol.into(),
            },
        }
    }

    fn ports(process: &PortProcess) -> Vec<(&str, u16, &str)> {
        process
            .ports
            .iter()
            .map(|binding| {
                (
                    binding.address.as_str(),
                    binding.port,
                    binding.protocol.as_str(),
                )
            })
            .collect()
    }

    fn assembled(raw: RawScan) -> Vec<PortProcess> {
        assemble(raw, |_| {})
    }

    #[test]
    fn a_process_is_one_row_with_each_socket_once() {
        let processes = assembled(RawScan {
            sockets: vec![
                socket(42, "node", "*", 3000, "TCP"),
                // The IPv4 and IPv6 sockets of one wildcard listener.
                socket(42, "node", "*", 3000, "TCP"),
                socket(42, "node", "127.0.0.1", 9229, "TCP"),
                socket(42, "node", "*", 3000, "UDP"),
            ],
            details: HashMap::new(),
        });

        assert_eq!(processes.len(), 1);
        assert_eq!(
            ports(&processes[0]),
            vec![
                ("*", 3000, "TCP"),
                ("127.0.0.1", 9229, "TCP"),
                ("*", 3000, "UDP")
            ]
        );
    }

    #[test]
    fn sockets_without_an_owner_stay_separate_rows() {
        let processes = assembled(RawScan {
            sockets: vec![
                socket(0, "unknown", "0.0.0.0", 443, "TCP"),
                socket(0, "unknown", "0.0.0.0", 80, "TCP"),
                socket(0, "unknown", "0.0.0.0", 80, "TCP"),
            ],
            details: HashMap::new(),
        });

        let ids: Vec<&str> = processes
            .iter()
            .map(|process| process.id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "socket-tcp-0.0.0.0-80",
                "socket-tcp-0.0.0.0-80#2",
                "socket-tcp-0.0.0.0-443"
            ]
        );
    }

    #[test]
    fn details_are_joined_by_pid_and_rows_come_out_sorted() {
        let details = HashMap::from([
            (
                7,
                ProcessDetails {
                    user: "dev".into(),
                    command_line: "node /srv/app/server.js --port 8080".into(),
                    working_directory: "/srv/app".into(),
                    executable_path: "/usr/bin/node".into(),
                    started_at: 1_790_000_000,
                    delete_blocked: Some("not a project".into()),
                },
            ),
            (9, ProcessDetails::default()),
        ]);
        let processes = assembled(RawScan {
            sockets: vec![
                socket(7, "node", "*", 8080, "TCP"),
                socket(9, "nginx", "*", 80, "TCP"),
                socket(5, "ghost", "*", 81, "TCP"),
            ],
            details,
        });

        let rows: Vec<(&str, u32)> = processes
            .iter()
            .map(|process| (process.id.as_str(), process.pid))
            .collect();
        assert_eq!(rows, vec![("pid-9", 9), ("pid-5", 5), ("pid-7", 7)]);

        let node = &processes[2];
        assert_eq!(node.user, "dev");
        assert_eq!(node.script_path.as_deref(), Some("/srv/app/server.js"));
        assert_eq!(node.working_directory, "/srv/app");
        assert_eq!(node.project_root, "/srv/app");
        assert_eq!(node.started_at, 1_790_000_000);
        assert_eq!(node.delete_blocked.as_deref(), Some("not a project"));

        // A process nothing is known about still gets its row.
        let ghost = &processes[1];
        assert_eq!((ghost.name.as_str(), ghost.user.as_str()), ("ghost", ""));
        assert_eq!(ghost.started_at, 0);
    }

    // With no working directory, the project is looked for from the script,
    // and failing that from the executable.
    #[test]
    fn the_project_root_falls_back_to_the_script_then_the_executable() {
        let details = HashMap::from([
            (
                1,
                ProcessDetails {
                    command_line: "python /srv/api/main.py".into(),
                    executable_path: "/usr/bin/python3".into(),
                    ..Default::default()
                },
            ),
            (
                2,
                ProcessDetails {
                    executable_path: "/opt/tool/bin/tool".into(),
                    ..Default::default()
                },
            ),
        ]);
        let processes = assembled(RawScan {
            sockets: vec![
                socket(1, "python", "*", 1, "TCP"),
                socket(2, "tool", "*", 2, "TCP"),
            ],
            details,
        });

        assert_eq!(processes[0].project_root, "/srv/api/main.py");
        assert_eq!(processes[1].project_root, "/opt/tool/bin/tool");
    }

    #[test]
    fn every_row_is_classified() {
        let processes = assemble(
            RawScan {
                sockets: vec![socket(1, "a", "*", 1, "TCP"), socket(0, "b", "*", 2, "TCP")],
                details: HashMap::new(),
            },
            |process| {
                process.system_kind = SystemKind::System;
                process.is_system_service = true;
            },
        );
        assert!(processes.iter().all(|process| process.is_system_service));
    }

    // The first name a platform gives may be empty (a process seen before
    // its name could be read); a later socket's name fills it in.
    #[test]
    fn an_empty_name_is_filled_in_by_a_later_socket() {
        let processes = assembled(RawScan {
            sockets: vec![
                socket(3, "", "*", 1, "TCP"),
                socket(3, "nginx", "*", 2, "TCP"),
            ],
            details: HashMap::new(),
        });
        assert_eq!(processes[0].name, "nginx");
    }

    fn listener(pid: u32, address: &str, port: u16) -> PortProcess {
        PortProcess {
            id: String::new(),
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
    fn every_row_gets_its_own_id() {
        let mut processes = vec![
            listener(0, "0.0.0.0", 22),
            listener(0, "[::]", 22),
            listener(0, "0.0.0.0", 443),
            // Two sockets that cannot be told apart at all.
            listener(0, "0.0.0.0", 443),
            listener(100, "*", 80),
            listener(200, "*", 80),
        ];
        sort_processes(&mut processes);
        assign_ids(&mut processes);

        let ids: Vec<&str> = processes
            .iter()
            .map(|process| process.id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "socket-tcp-0.0.0.0-22",
                "socket-tcp-[::]-22",
                "pid-100",
                "pid-200",
                "socket-tcp-0.0.0.0-443",
                "socket-tcp-0.0.0.0-443#2",
            ]
        );
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
