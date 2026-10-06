//! What `scan.ps1` prints: a JSON array with one object per listening
//! socket, each carrying the process that owns it.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Listener {
    pub pid: u32,
    pub name: String,
    pub user: String,
    pub local_address: String,
    pub local_port: u16,
    pub executable_path: Option<String>,
    pub command_line: Option<String>,
    pub protocol: String,
    // Unix seconds, 0 when unknown. Signed so a clock far in the past cannot
    // fail the whole parse; clamped where it is used.
    pub started_at: i64,
}

pub fn parse(stdout: &str) -> Result<Vec<Listener>, String> {
    let json = stdout.trim();
    if json.is_empty() {
        return Ok(Vec::new());
    }

    // The script always prints an array. A lone object is what PowerShell's
    // ConvertTo-Json makes of a one-item pipeline, so it is read too.
    if json.starts_with('[') {
        serde_json::from_str(json)
    } else {
        serde_json::from_str(json).map(|listener| vec![listener])
    }
    .map_err(|e| format!("Failed to parse PowerShell JSON: {e}"))
}

/// The form the other platforms use: `*` for every interface, and IPv6 in
/// brackets.
pub fn normalize_address(address: &str) -> String {
    if address == "0.0.0.0" || address == "::" {
        "*".to_string()
    } else if address.contains(':') && !address.starts_with('[') {
        format!("[{address}]")
    } else {
        address.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::super::fixture;
    use super::*;

    #[test]
    fn reads_an_array_of_listeners() {
        let listeners = parse(fixture!("powershell-array.json")).unwrap();
        assert_eq!(listeners.len(), 4);

        let node = &listeners[0];
        assert_eq!(node.pid, 4242);
        assert_eq!(node.name, "node.exe");
        assert_eq!(node.user, "DESKTOP-7Q2\\dev");
        assert_eq!(node.local_address, "::");
        assert_eq!(node.local_port, 3000);
        assert_eq!(
            node.executable_path.as_deref(),
            Some("C:\\Program Files\\nodejs\\node.exe")
        );
        assert_eq!(
            node.command_line.as_deref(),
            Some("\"C:\\Program Files\\nodejs\\node.exe\" C:\\Users\\dev\\app\\server.js")
        );
        assert_eq!(node.protocol, "TCP");
        assert_eq!(node.started_at, 1_790_000_000);

        // The System process: no path, no command line, no start time.
        let system = &listeners[2];
        assert_eq!((system.pid, system.local_port), (4, 445));
        assert_eq!(system.executable_path.as_deref(), Some(""));
        assert_eq!(system.started_at, 0);

        // PowerShell prints an empty value as null.
        let svchost = &listeners[3];
        assert_eq!(svchost.protocol, "UDP");
        assert_eq!(svchost.executable_path, None);
        assert_eq!(svchost.command_line, None);
    }

    #[test]
    fn reads_a_lone_object() {
        let listeners = parse(fixture!("powershell-single.json")).unwrap();
        assert_eq!(listeners.len(), 1);
        assert_eq!(listeners[0].pid, 9120);
        assert_eq!(listeners[0].local_address, "127.0.0.1");
    }

    #[test]
    fn no_output_is_no_listeners() {
        assert!(parse("").unwrap().is_empty());
        assert!(parse(" \r\n").unwrap().is_empty());
    }

    #[test]
    fn output_that_is_not_json_is_an_error() {
        let error = parse("Get-NetTCPConnection : Access denied").unwrap_err();
        assert!(
            error.starts_with("Failed to parse PowerShell JSON"),
            "{error}"
        );
    }

    #[test]
    fn a_start_time_before_1970_does_not_fail_the_scan() {
        let json = r#"[{"pid":1,"name":"a","user":"","localAddress":"::1","localPort":1,"executablePath":"","commandLine":"","protocol":"TCP","startedAt":-11644473600}]"#;
        assert_eq!(parse(json).unwrap()[0].started_at, -11_644_473_600);
    }

    #[test]
    fn normalize_address_matches_the_other_platforms() {
        assert_eq!(normalize_address("0.0.0.0"), "*");
        assert_eq!(normalize_address("::"), "*");
        assert_eq!(normalize_address("127.0.0.1"), "127.0.0.1");
        assert_eq!(normalize_address("::1"), "[::1]");
        assert_eq!(normalize_address("fe80::1%4"), "[fe80::1%4]");
        assert_eq!(normalize_address("[::1]"), "[::1]");
    }
}
