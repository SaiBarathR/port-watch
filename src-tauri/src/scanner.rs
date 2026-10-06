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
    mark_undeletable_folders(&mut processes, DeleteRules::for_current_user());
    Ok(processes)
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
    use crate::platform::shared::parse_address_port;

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
