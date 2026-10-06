//! `ss -H -tlnp` and `ss -H -ulnp` output: one socket per line.

use crate::platform::shared::parse_address_port;
use crate::scanner::RawSocket;

pub fn parse_listeners(stdout: &str, protocol: &str) -> Vec<RawSocket> {
    let mut sockets = Vec::new();

    for line in stdout.lines() {
        let Some((owners, local)) = parse_line(line.trim()) else {
            continue;
        };
        let Some(binding) = parse_address_port(local, protocol) else {
            continue;
        };

        // Pre-forked servers (nginx, gunicorn) share one listening socket
        // between a master and its workers. ss names every owner on the one
        // line, and each is a process the user may want to stop.
        for (pid, name) in owners {
            sockets.push(RawSocket {
                pid,
                name,
                binding: binding.clone(),
            });
        }
    }

    sockets
}

fn parse_line(line: &str) -> Option<(Vec<(u32, String)>, &str)> {
    // ss only adds the `users:(...)` field for sockets the caller owns (all
    // of them when run as root). A socket without it still listens, so it is
    // kept under PID 0 instead of being dropped from the scan.
    let (before_users, users_part) = match line.find("users:") {
        Some(start) => (line[..start].trim(), Some(&line[start..])),
        None => (line, None),
    };

    let owners = match users_part {
        Some(users_part) => {
            let owners = parse_owners(users_part);
            if owners.is_empty() {
                return None;
            }
            owners
        }
        None => vec![(0, "unknown".to_string())],
    };

    let parts: Vec<&str> = before_users.split_whitespace().collect();
    let local = *parts.get(parts.len().checked_sub(2)?)?;
    Some((owners, local))
}

// Every ("name",pid=N,fd=M) owner in a users:(...) field.
fn parse_owners(users_part: &str) -> Vec<(u32, String)> {
    let mut owners = Vec::new();

    for segment in users_part.split("(\"").skip(1) {
        let name = segment.split('"').next().unwrap_or("unknown").to_string();
        let pid = segment
            .split("pid=")
            .nth(1)
            .and_then(|rest| rest.split([',', ')']).next())
            .and_then(|value| value.parse().ok());
        if let Some(pid) = pid {
            owners.push((pid, name));
        }
    }

    owners
}

#[cfg(test)]
mod tests {
    use super::super::fixture;
    use super::*;

    fn summary(sockets: &[RawSocket]) -> Vec<(u32, &str, &str, u16)> {
        sockets
            .iter()
            .map(|socket| {
                (
                    socket.pid,
                    socket.name.as_str(),
                    socket.binding.address.as_str(),
                    socket.binding.port,
                )
            })
            .collect()
    }

    // Captured as root: a forked pair sharing port 80, a socket whose owner
    // was not reported, a loopback listener and a dual-stack wildcard.
    #[test]
    fn reads_tcp_listeners_as_root() {
        let sockets = parse_listeners(fixture!("ss-tcp-root.txt"), "TCP");
        assert_eq!(
            summary(&sockets),
            vec![
                (23, "python3", "0.0.0.0", 80),
                (21, "python3", "0.0.0.0", 80),
                (0, "unknown", "0.0.0.0", 9000),
                (14, "python3", "127.0.0.1", 8000),
                (16, "python3", "*", 8080),
            ]
        );
    }

    // The same machine seen by an unprivileged user: only its own socket
    // comes with an owner, and the rest are still listed.
    #[test]
    fn reads_tcp_listeners_without_privileges() {
        let sockets = parse_listeners(fixture!("ss-tcp-unprivileged.txt"), "TCP");
        assert_eq!(
            summary(&sockets),
            vec![
                (0, "unknown", "0.0.0.0", 80),
                (19, "python3", "0.0.0.0", 9000),
                (0, "unknown", "127.0.0.1", 8000),
                (0, "unknown", "*", 8080),
            ]
        );
    }

    #[test]
    fn reads_udp_sockets() {
        let sockets = parse_listeners(fixture!("ss-udp-root.txt"), "UDP");
        assert_eq!(
            summary(&sockets),
            vec![
                (23, "python3", "0.0.0.0", 5353),
                (21, "python3", "0.0.0.0", 5353),
                (23, "python3", "[::1]", 5354),
                (21, "python3", "[::1]", 5354),
            ]
        );
        assert!(sockets
            .iter()
            .all(|socket| socket.binding.protocol == "UDP"));
    }

    #[test]
    fn the_state_column_is_optional() {
        let with_state = "LISTEN 0 4096 127.0.0.1:8080 0.0.0.0:* users:((\"node\",pid=1234,fd=21))";
        let without = "0 4096 [::1]:3000 0.0.0.0:* users:((\"node\",pid=5678,fd=3))";
        assert_eq!(
            parse_line(with_state),
            Some((vec![(1234, "node".to_string())], "127.0.0.1:8080"))
        );
        assert_eq!(
            parse_line(without),
            Some((vec![(5678, "node".to_string())], "[::1]:3000"))
        );
    }

    #[test]
    fn a_line_that_is_not_a_socket_is_skipped() {
        assert!(parse_listeners("\n   \nNetid State\n", "TCP").is_empty());
        // A users field nobody could be read from.
        assert_eq!(
            parse_line("LISTEN 0 5 0.0.0.0:80 0.0.0.0:* users:(garbled)"),
            None
        );
    }
}
