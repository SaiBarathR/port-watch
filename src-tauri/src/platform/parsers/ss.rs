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

    // A users field that does not read as one is not trusted for a PID: the
    // socket is listed like one whose owner was not reported, which can be
    // seen and cannot be stopped.
    let nobody = || vec![(0, "unknown".to_string())];
    let owners = match users_part {
        Some(users_part) => parse_owners(users_part.trim_end()).unwrap_or_else(nobody),
        None => nobody(),
    };

    let parts: Vec<&str> = before_users.split_whitespace().collect();
    let local = *parts.get(parts.len().checked_sub(2)?)?;
    Some((owners, local))
}

// Every ("name",pid=N,fd=M) owner in a users:((...),(...)) field, or None if
// the field is not made of such entries from end to end.
//
// A process chooses its own name, and ss prints it as it is: up to 15 bytes
// that may hold quotes, commas and brackets. Taking the name up to the first
// quote and the PID after the first `pid=` let a process named
// `sh",pid=4242,` be listed as PID 4242, a process the user would then stop
// in its place. So a name ends only where the rest of an entry follows and,
// after it, either the next entry or the end of the field. Forging that
// inside a name takes `",pid=1,fd=1),("`, which is 16 bytes.
fn parse_owners(users_part: &str) -> Option<Vec<(u32, String)>> {
    let mut rest = users_part.strip_prefix("users:(")?;
    let mut owners = Vec::new();

    loop {
        let (name, pid, after) = split_entry(rest.strip_prefix("(\"")?)?;
        owners.push((pid, name.to_string()));
        if after == ")" {
            return Some(owners);
        }
        rest = after.strip_prefix(',')?;
    }
}

// `name",pid=N,fd=M)…` as the name, the PID and what follows the entry, cut
// at the first place the name can end.
fn split_entry(entry: &str) -> Option<(&str, u32, &str)> {
    entry.match_indices("\",pid=").find_map(|(at, mark)| {
        let (pid, after) = leading_number(&entry[at + mark.len()..])?;
        let (_, after) = leading_number(after.strip_prefix(",fd=")?)?;
        let after = after.strip_prefix(')')?;
        (after == ")" || after.starts_with(",(\"")).then_some((&entry[..at], pid, after))
    })
}

fn leading_number(text: &str) -> Option<(u32, &str)> {
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    Some((text[..digits].parse().ok()?, &text[digits..]))
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
    }

    // The socket is still listening, so it is listed, as one nobody owns.
    #[test]
    fn a_users_field_that_cannot_be_read_names_nobody() {
        for users in [
            "users:(garbled)",
            "users:((\"node\",pid=12,fd=3)",
            "users:((\"node\",pid=12,fd=3)) extra",
            "users:((\"node\",pid=,fd=3))",
            "users:((\"node\",pid=12))",
        ] {
            assert_eq!(
                parse_line(&format!("LISTEN 0 5 0.0.0.0:80 0.0.0.0:* {users}")),
                Some((vec![(0, "unknown".to_string())], "0.0.0.0:80")),
                "{users}"
            );
        }
    }

    // A process picks its own name, up to 15 bytes of it, and ss prints it
    // as it is. None of these may come out as PID 1.
    #[test]
    fn a_name_cannot_pass_itself_off_as_another_pid() {
        for name in [
            r#"systemd",pid=1,"#,
            r#"sh",pid=1,fd=3)"#,
            r#"x",pid=1,fd=3))"#,
            r#"),("x",pid=1,"#,
            r#"a("b"#,
            r#"users:(("a","#,
        ] {
            assert!(name.len() <= 15, "{name}");
            let alone = format!(r#"LISTEN 0 5 *:80 *:* users:(("{name}",pid=4321,fd=7))"#);
            assert_eq!(
                parse_line(&alone),
                Some((vec![(4321, name.to_string())], "*:80")),
                "{name}"
            );

            let shared = format!(
                r#"LISTEN 0 5 *:80 *:* users:(("{name}",pid=4321,fd=7),("nginx",pid=77,fd=6))"#
            );
            assert_eq!(
                parse_line(&shared),
                Some((
                    vec![(4321, name.to_string()), (77, "nginx".to_string())],
                    "*:80"
                )),
                "{name}"
            );
        }
    }
}
