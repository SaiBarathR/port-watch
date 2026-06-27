pub fn extract_script_path(command_line: &str, process_name: &str) -> Option<String> {
    if command_line.is_empty() {
        return None;
    }

    let interpreters = [
        "python", "python3", "node", "nodejs", "bun", "ruby", "perl", "php", "java",
    ];

    let name_lower = process_name.to_lowercase();
    let is_interpreter = interpreters.iter().any(|i| name_lower.contains(i));

    let tokens: Vec<&str> = command_line.split_whitespace().collect();
    let start_idx = if is_interpreter { 1 } else { 0 };

    for token in tokens.iter().skip(start_idx) {
        let cleaned = token.trim_matches('"').trim_matches('\'');
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
}
