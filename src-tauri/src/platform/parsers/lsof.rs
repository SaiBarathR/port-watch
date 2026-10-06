//! `lsof -F` output: one field per line, led by a letter. `p` opens a
//! process and `f` one of its files; `c` and `n` name them.

use std::collections::HashMap;

use crate::platform::shared::parse_address_port;
use crate::scanner::RawSocket;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProcessPaths {
    pub working_directory: String,
    pub executable_path: String,
}

fn field(line: &str) -> Option<(char, &str)> {
    let tag = line.chars().next()?;
    Some((tag, &line[tag.len_utf8()..]))
}

/// From `lsof -i<protocol> -n -P -F pcn`. A socket whose process has no
/// readable PID or name is left out: nothing could be done with it.
pub fn parse_listeners(stdout: &str, protocol: &str) -> Vec<RawSocket> {
    let mut sockets = Vec::new();
    let mut pid: Option<u32> = None;
    let mut name: Option<String> = None;

    for line in stdout.lines() {
        match field(line) {
            Some(('p', value)) => {
                pid = value.parse().ok();
                name = None;
            }
            Some(('c', value)) => name = Some(unescape(value)),
            Some(('n', value)) => {
                let (Some(pid), Some(name)) = (pid, &name) else {
                    continue;
                };
                if let Some(binding) = parse_address_port(value, protocol) {
                    sockets.push(RawSocket {
                        pid,
                        name: name.clone(),
                        binding,
                    });
                }
            }
            _ => {}
        }
    }

    sockets
}

/// From `lsof -a -p <pids> -d cwd,txt -Fn`.
pub fn parse_paths(stdout: &str) -> HashMap<u32, ProcessPaths> {
    let mut paths: HashMap<u32, ProcessPaths> = HashMap::new();
    let mut pid: Option<u32> = None;
    let mut descriptor = "";

    for line in stdout.lines() {
        match field(line) {
            Some(('p', value)) => {
                pid = value.parse().ok();
                descriptor = "";
            }
            Some(('f', value)) => descriptor = value,
            Some(('n', value)) => {
                let Some(pid) = pid else {
                    continue;
                };
                let entry = paths.entry(pid).or_default();
                match descriptor {
                    "cwd" => entry.working_directory = unescape(value),
                    // The executable is the first txt entry; the libraries
                    // and caches it has mapped follow.
                    "txt" if entry.executable_path.is_empty() => {
                        entry.executable_path = unescape(value);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    paths
}

// lsof escapes a backslash as `\\` and, when the app runs without a locale
// (launched from Finder or the Dock), every non-ASCII byte as `\xNN`. Undoing
// both yields the kernel's own name for a process, which is what the stop
// path checks a PID against, and a path that exists on disk.
fn unescape(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut unescaped = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'\\' {
            match bytes.get(index + 1) {
                Some(b'\\') => {
                    unescaped.push(b'\\');
                    index += 2;
                    continue;
                }
                Some(b'x') => {
                    let byte = bytes
                        .get(index + 2..index + 4)
                        .filter(|hex| hex.iter().all(u8::is_ascii_hexdigit))
                        .and_then(|hex| std::str::from_utf8(hex).ok())
                        .and_then(|hex| u8::from_str_radix(hex, 16).ok());
                    if let Some(byte) = byte {
                        unescaped.push(byte);
                        index += 4;
                        continue;
                    }
                }
                _ => {}
            }
        }
        unescaped.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&unescaped).into_owned()
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

    #[test]
    fn reads_tcp_listeners_with_every_address_form() {
        let sockets = parse_listeners(fixture!("lsof-tcp-listen.txt"), "TCP");
        assert_eq!(
            summary(&sockets),
            vec![
                (612, "ControlCenter", "*", 7000),
                (612, "ControlCenter", "*", 7000),
                (612, "ControlCenter", "*", 5000),
                (612, "ControlCenter", "*", 5000),
                (1006, "rapportd", "*", 49152),
                (4310, "postgres", "127.0.0.1", 5432),
                (4310, "postgres", "[::1]", 5432),
                (5120, "Google Chrome Helper", "127.0.0.1", 9222),
                (6001, "Café Sërver", "[fe80:4::1c2d:3eff:fe4f:5a6b]", 8443),
                (7777, "back\\slash", "192.168.1.20", 3000),
            ]
        );
        assert!(sockets
            .iter()
            .all(|socket| socket.binding.protocol == "TCP"));
    }

    // UDP lists every socket, bound or not: `*:*` has no port, and a
    // connected socket (`a->b`) is a client's, not a listener.
    #[test]
    fn reads_udp_and_leaves_out_what_is_not_a_listener() {
        let sockets = parse_listeners(fixture!("lsof-udp.txt"), "UDP");
        assert_eq!(
            summary(&sockets),
            vec![
                (612, "ControlCenter", "*", 5353),
                (1006, "rapportd", "*", 3722),
                (1006, "rapportd", "192.168.1.20", 61234),
            ]
        );
        assert!(sockets
            .iter()
            .all(|socket| socket.binding.protocol == "UDP"));
    }

    #[test]
    fn a_socket_without_a_pid_or_a_name_is_left_out() {
        assert!(parse_listeners("pnot-a-pid\ncnode\nf12\nn*:3000\n", "TCP").is_empty());
        assert!(parse_listeners("p42\nf12\nn*:3000\n", "TCP").is_empty());
        // And it does not lend its sockets to the process after it.
        let sockets = parse_listeners("px\ncghost\nn*:1\np42\ncnode\nn*:3000\n", "TCP");
        assert_eq!(summary(&sockets), vec![(42, "node", "*", 3000)]);
    }

    #[test]
    fn a_line_that_starts_with_a_wide_character_is_skipped() {
        let sockets = parse_listeners("p42\ncnode\né:1\nn*:3000\n", "TCP");
        assert_eq!(summary(&sockets), vec![(42, "node", "*", 3000)]);
    }

    #[test]
    fn reads_the_working_directory_and_the_first_txt_entry() {
        let paths = parse_paths(fixture!("lsof-paths.txt"));
        assert_eq!(paths.len(), 3);
        assert_eq!(
            paths[&1006],
            ProcessPaths {
                working_directory: "/".into(),
                executable_path: "/usr/libexec/rapportd".into(),
            }
        );
        assert_eq!(
            paths[&4310],
            ProcessPaths {
                working_directory: "/Users/dev/projects/café app".into(),
                executable_path: "/opt/homebrew/Cellar/postgresql@16/16.4/bin/postgres".into(),
            }
        );
        // A process whose working directory could not be read.
        assert_eq!(
            paths[&7777],
            ProcessPaths {
                working_directory: String::new(),
                executable_path: "/Users/dev/bin/back\\slash".into(),
            }
        );
    }

    #[test]
    fn unescape_restores_the_kernels_bytes() {
        assert_eq!(unescape("node"), "node");
        assert_eq!(unescape("Caf\\xc3\\xa9 S\\xc3\\xabrver"), "Café Sërver");
        assert_eq!(unescape("back\\\\slash name"), "back\\slash name");
        // An escaped backslash followed by `x41` is not the byte 0x41.
        assert_eq!(unescape("a\\\\x41"), "a\\x41");
        // Not escapes lsof produces: left as they are.
        assert_eq!(unescape("a\\xzz"), "a\\xzz");
        assert_eq!(unescape("trailing\\"), "trailing\\");
    }
}
