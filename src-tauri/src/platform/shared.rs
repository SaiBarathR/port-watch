use std::io::{self, Read};
use std::process::{Command, ExitStatus, Output, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

/// How long one scan tool (lsof, ps, ss, ...) may run. They normally finish in
/// tens of milliseconds, so this only ends a tool that is stuck.
#[cfg_attr(target_os = "windows", allow(dead_code))]
pub const SCAN_COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

/// `Command::output` with a deadline. A scan runs its tools one after another
/// on one worker, so a tool that hangs (lsof on a dead network mount) would
/// stall every scan after it. On timeout the child is killed and reaped.
pub fn run_with_timeout(command: &mut Command, timeout: Duration) -> io::Result<Output> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = read_in_background(child.stdout.take());
    let stderr = read_in_background(child.stderr.take());
    let deadline = Instant::now() + timeout;

    // Both pipes closing is what a finished child looks like from here, and
    // waiting for that takes no polling.
    let finished = receive_by(&stdout, deadline)
        .zip(receive_by(&stderr, deadline))
        .and_then(|(stdout, stderr)| {
            let status = wait_by(&mut child, deadline)?;
            Some(Output {
                status,
                stdout,
                stderr,
            })
        });

    finished.ok_or_else(|| {
        // The readers are left to end on their own: a grandchild may still
        // hold the pipes open.
        let _ = child.kill();
        let _ = child.wait();
        io::Error::new(
            io::ErrorKind::TimedOut,
            format!(
                "{} did not finish within {} s",
                command.get_program().to_string_lossy(),
                timeout.as_secs()
            ),
        )
    })
}

fn read_in_background<R: Read + Send + 'static>(pipe: Option<R>) -> Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        let _ = sender.send(bytes);
    });
    receiver
}

fn receive_by(receiver: &Receiver<Vec<u8>>, deadline: Instant) -> Option<Vec<u8>> {
    receiver
        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
        .ok()
}

// The pipes are already closed, so the exit is normally immediate.
fn wait_by(child: &mut std::process::Child, deadline: Instant) -> Option<ExitStatus> {
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            return Some(status);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

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
        if (cleaned.contains('/') || cleaned.contains('\\')) && !cleaned.starts_with("/dev/") {
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
// macOS reads the kernel's name on both sides and compares it exactly.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn process_names_match(current: &str, expected: &str) -> bool {
    fn normalize(name: &str) -> String {
        let base = name
            .trim()
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(name)
            .to_ascii_lowercase();
        base.strip_suffix(".exe")
            .map(str::to_string)
            .unwrap_or(base)
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

// Windows gets structured addresses from PowerShell and never parses one.
#[cfg_attr(target_os = "windows", allow(dead_code))]
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
    #[cfg(unix)]
    fn run_with_timeout_returns_output_and_status() {
        let output = run_with_timeout(
            Command::new("sh").args(["-c", "printf out; printf err >&2; exit 3"]),
            Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(output.stdout, b"out");
        assert_eq!(output.stderr, b"err");
        assert_eq!(output.status.code(), Some(3));
    }

    #[test]
    #[cfg(unix)]
    fn run_with_timeout_handles_more_output_than_a_pipe_holds() {
        let output = run_with_timeout(
            Command::new("sh").args(["-c", "head -c 1000000 /dev/zero"]),
            Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(output.stdout.len(), 1_000_000);
        assert!(output.status.success());
    }

    #[test]
    #[cfg(unix)]
    fn run_with_timeout_kills_and_reaps_a_hung_child() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("pid");

        let started = Instant::now();
        let error = run_with_timeout(
            Command::new("sh")
                .args(["-c", "echo $$ > \"$0\"; exec sleep 60"])
                .arg(&pid_file),
            Duration::from_secs(1),
        )
        .unwrap_err();
        let elapsed = started.elapsed();

        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(error.to_string().contains("sh did not finish"), "{error}");
        assert!(elapsed < Duration::from_secs(10), "took {elapsed:?}");

        // Neither running nor left behind as a zombie (`ps` would print Z).
        let pid = std::fs::read_to_string(&pid_file).unwrap();
        let ps = Command::new("ps")
            .args(["-o", "stat=", "-p", pid.trim()])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&ps.stdout).trim(), "");
    }

    #[test]
    #[cfg(windows)]
    fn run_with_timeout_kills_a_hung_child() {
        let started = Instant::now();
        let error = run_with_timeout(
            Command::new("powershell").args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 60",
            ]),
            Duration::from_secs(2),
        )
        .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(30));
    }

    #[test]
    fn run_with_timeout_reports_a_missing_program() {
        let error = run_with_timeout(
            &mut Command::new("port-watch-no-such-program"),
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

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
        assert_eq!(
            extract_script_path("/usr/sbin/nginx -g daemon", "nginx"),
            None
        );
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
