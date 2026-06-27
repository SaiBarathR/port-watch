use serde::{Deserialize, Serialize};

use crate::platform;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortBinding {
    pub address: String,
    pub port: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub uptime_seconds: u64,
}

pub fn scan_listening_ports(include_udp: bool) -> Result<Vec<PortProcess>, String> {
    platform::scan_listening_ports(include_udp)
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
