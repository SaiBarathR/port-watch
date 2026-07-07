pub fn extract_script_path(command_line: &str, process_name: &str) -> Option<String> {
    if command_line.is_empty() {
        return None;
    }

    let interpreters = [
        "python", "python3", "node", "nodejs", "bun", "ruby", "perl", "php", "java",
    ];

    let name_lower = process_name.to_lowercase();
    let is_interpreter = interpreters.iter().any(|i| name_lower.contains(i));
    if !is_interpreter {
        // For non-interpreter processes the first path-like token is argv0
        // (the executable itself), not a script.
        return None;
    }

    let tokens = tokenize(command_line);

    for token in tokens.iter().skip(1) {
        let cleaned = token.as_str();
        if cleaned.starts_with('-') {
            continue;
        }
        if (cleaned.contains('/') || cleaned.contains('\\'))
            && !cleaned.starts_with("/dev/")
        {
            return Some(cleaned.to_string());
        }
    }

    None
}

/// Split a command line into tokens, honoring single/double quotes so that a
/// quoted path containing spaces (e.g. `"C:\Program Files\node.exe"`) stays a
/// single token instead of being shattered on the embedded space.
fn tokenize(command_line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;

    for ch in command_line.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => current.push(ch),
            None if ch == '"' || ch == '\'' => quote = Some(ch),
            None if ch.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            None => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

/// Compare a live process name against the name the user saw in the last
/// scan, tolerating truncation (macOS/Linux report at most ~15 chars), a
/// Windows `.exe` suffix, and full-path vs basename differences. Used to
/// refuse killing a PID that has been reused by a different process.
pub fn process_names_match(current: &str, expected: &str) -> bool {
    fn normalize(name: &str) -> String {
        let base = name
            .trim()
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(name)
            .to_ascii_lowercase();
        base.strip_suffix(".exe").map(str::to_string).unwrap_or(base)
    }

    let current = normalize(current);
    let expected = normalize(expected);
    if current.is_empty() || expected.is_empty() {
        return false;
    }
    if current == expected {
        return true;
    }

    // Kernel-truncated names still match their long form, but only at the
    // actual truncation boundaries (15 chars on Linux comm, 16 on macOS
    // MAXCOMLEN) — a longer shared prefix means genuinely different names.
    let (short, long) = if current.len() < expected.len() {
        (&current, &expected)
    } else {
        (&expected, &current)
    };
    (15..=16).contains(&short.len()) && long.starts_with(short.as_str())
}

pub fn parse_address_port(value: &str, protocol: &str) -> Option<crate::scanner::PortBinding> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if protocol.eq_ignore_ascii_case("UDP") && value.contains("->") {
        return None;
    }

    let (address, port_str) = if value.starts_with('[') {
        let end = value.rfind("]:")?;
        let address = value[..=end].to_string();
        let port_str = &value[end + 2..];
        (address, port_str)
    } else if let Some((addr, port)) = value.rsplit_once(':') {
        if addr.is_empty() {
            return None;
        }
        (addr.to_string(), port)
    } else {
        return None;
    };

    let port: u16 = port_str.parse().ok()?;

    Some(crate::scanner::PortBinding {
        address,
        port,
        protocol: protocol.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ipv4() {
        let binding = parse_address_port("127.0.0.1:8090", "TCP").unwrap();
        assert_eq!(binding.address, "127.0.0.1");
        assert_eq!(binding.port, 8090);
    }

    #[test]
    fn parse_wildcard() {
        let binding = parse_address_port("*:8090", "TCP").unwrap();
        assert_eq!(binding.address, "*");
        assert_eq!(binding.port, 8090);
    }

    #[test]
    fn parse_ipv6_bracketed() {
        let binding = parse_address_port("[::1]:8080", "TCP").unwrap();
        assert_eq!(binding.address, "[::1]");
        assert_eq!(binding.port, 8080);
    }

    #[test]
    fn parse_ipv6_with_zone() {
        let binding = parse_address_port("[fe80::1%en0]:5353", "UDP").unwrap();
        assert_eq!(binding.address, "[fe80::1%en0]");
        assert_eq!(binding.port, 5353);
    }

    #[test]
    fn skips_connected_udp_socket() {
        assert!(parse_address_port("192.168.1.10:54321->8.8.8.8:53", "UDP").is_none());
    }

    #[test]
    fn extract_script_path_quoted_interpreter_with_spaces() {
        // The interpreter path contains spaces and is quoted; the real script
        // is the following argument. The interpreter token must not be split.
        let cmd = "\"C:\\Program Files\\nodejs\\node.exe\" C:\\app\\server.js";
        assert_eq!(
            extract_script_path(cmd, "node"),
            Some("C:\\app\\server.js".to_string())
        );
    }

    #[test]
    fn extract_script_path_unix_interpreter() {
        let cmd = "node /srv/app/server.js --port 3000";
        assert_eq!(
            extract_script_path(cmd, "node"),
            Some("/srv/app/server.js".to_string())
        );
    }

    #[test]
    fn extract_script_path_ignores_non_interpreters() {
        assert_eq!(extract_script_path("/usr/sbin/nginx -g daemon", "nginx"), None);
    }

    #[test]
    fn process_names_match_variants() {
        assert!(process_names_match("node", "node"));
        assert!(process_names_match("Node", "node"));
        assert!(process_names_match("/usr/local/bin/node", "node"));
        assert!(process_names_match("node.exe", "node"));
        assert!(process_names_match(
            "com.docker.backe",
            "com.docker.backend"
        ));
        assert!(!process_names_match("nginx", "node"));
        assert!(!process_names_match("node", "nodemon"));
        assert!(!process_names_match("", "node"));
        // A shared prefix longer than the kernel truncation lengths means two
        // genuinely different names.
        assert!(!process_names_match(
            "com.example.service",
            "com.example.service-worker"
        ));
    }
}
