use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::platform::parsers::{procfs, ss};
use crate::platform::shared::{run_with_timeout, SCAN_COMMAND_TIMEOUT};
use crate::scanner::{ProcessDetails, RawScan, RawSocket};

pub fn scan(include_udp: bool) -> Result<RawScan, String> {
    let mut sockets = list_sockets(&["-H", "-tlnp"], "TCP")?;
    if include_udp {
        sockets.extend(list_sockets(&["-H", "-ulnp"], "UDP")?);
    }

    let mut details = HashMap::new();
    for socket in &sockets {
        // PID 0 stands for an owner this user cannot see.
        if socket.pid != 0 {
            details
                .entry(socket.pid)
                .or_insert_with(|| read_details(socket.pid));
        }
    }

    Ok(RawScan { sockets, details })
}

fn list_sockets(args: &[&str], protocol: &str) -> Result<Vec<RawSocket>, String> {
    let output = run_with_timeout(Command::new("ss").args(args), SCAN_COMMAND_TIMEOUT)
        .map_err(|e| format!("Failed to run ss: {e}"))?;

    if !output.status.success() && output.stdout.is_empty() {
        return Err(format!(
            "ss exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(ss::parse_listeners(
        &String::from_utf8_lossy(&output.stdout),
        protocol,
    ))
}

// Whatever cannot be read (the process has exited, or is another user's) is
// left empty.
fn read_details(pid: u32) -> ProcessDetails {
    let proc_dir = Path::new("/proc").join(pid.to_string());
    // As bytes: a process name or an argument need not be UTF-8, and a file
    // that could not be read as text used to leave the process with no user
    // and no start time, which is how a system service looks.
    let read = |file: &str| {
        String::from_utf8_lossy(&fs::read(proc_dir.join(file)).unwrap_or_default()).into_owned()
    };
    let link = |file: &str| {
        fs::read_link(proc_dir.join(file))
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default()
    };

    ProcessDetails {
        user: procfs::parse_uid(&read("status"))
            .map(|uid| resolve_uid(uid).unwrap_or_else(|| uid.to_string()))
            .unwrap_or_default(),
        command_line: procfs::parse_cmdline(&read("cmdline")),
        working_directory: link("cwd"),
        executable_path: link("exe"),
        started_at: started_at_from_stat(&read("stat")),
        delete_blocked: None,
    }
}

fn resolve_uid(uid: u32) -> Option<String> {
    use std::sync::Mutex;

    static CACHE: Mutex<Option<HashMap<u32, Option<String>>>> = Mutex::new(None);

    if let Ok(mut guard) = CACHE.lock() {
        if let Some(cached) = guard.get_or_insert_with(HashMap::new).get(&uid) {
            return cached.clone();
        }
    }

    let resolved = resolve_uid_uncached(uid);

    if let Ok(mut guard) = CACHE.lock() {
        guard
            .get_or_insert_with(HashMap::new)
            .insert(uid, resolved.clone());
    }

    resolved
}

fn resolve_uid_uncached(uid: u32) -> Option<String> {
    let passwd = fs::read_to_string("/etc/passwd").unwrap_or_default();
    if let Some(name) = procfs::parse_passwd_name(&passwd, uid) {
        return Some(name);
    }

    // NSS-managed users (LDAP, SSSD, systemd-homed) aren't in /etc/passwd.
    let output = run_with_timeout(
        Command::new("getent").args(["passwd", &uid.to_string()]),
        SCAN_COMMAND_TIMEOUT,
    )
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let name = stdout.split(':').next()?.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

// When the process started, in Unix seconds, from the contents of
// /proc/<pid>/stat: the boot time plus the start offset the kernel records.
// Unlike an uptime it is the same on every scan. 0 when it cannot be worked
// out.
pub(super) fn started_at_from_stat(proc_pid_stat: &str) -> u64 {
    use std::sync::OnceLock;

    static CLOCK_TICKS: OnceLock<u64> = OnceLock::new();
    let clock_ticks = *CLOCK_TICKS.get_or_init(|| {
        run_with_timeout(Command::new("getconf").arg("CLK_TCK"), SCAN_COMMAND_TIMEOUT)
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|value| value.trim().parse().ok())
            .filter(|ticks| *ticks > 0)
            .unwrap_or(100)
    });

    // Kept once read: the kernel derives it from the wall clock, so it can
    // shift by a second after a clock adjustment and make every process look
    // new. A read that fails is not kept, and is tried again next time.
    static BOOT_TIME: OnceLock<u64> = OnceLock::new();
    let boot_time = match BOOT_TIME.get() {
        Some(boot_time) => *boot_time,
        None => {
            let read =
                procfs::parse_boot_time(&fs::read_to_string("/proc/stat").unwrap_or_default());
            let Some(boot_time) = read else {
                return 0;
            };
            *BOOT_TIME.get_or_init(|| boot_time)
        }
    };

    match procfs::parse_start_ticks(proc_pid_stat) {
        Some(start_ticks) => boot_time + start_ticks / clock_ticks,
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn started_at_is_stable_and_recent_for_this_process() {
        let stat = fs::read_to_string("/proc/self/stat").unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let first = started_at_from_stat(&stat);
        let second = started_at_from_stat(&stat);

        assert_eq!(first, second);
        assert!(first <= now + 1, "{first} is after {now}");
        assert!(now - first.min(now) < 3_600, "{first} is long before {now}");
    }

    #[test]
    fn details_of_this_process_are_read_from_proc() {
        let details = read_details(std::process::id());
        let exe = std::env::current_exe().unwrap();

        assert_eq!(details.executable_path, exe.to_string_lossy());
        assert!(details.command_line.contains(&*exe.to_string_lossy()));
        assert!(!details.user.is_empty());
        assert!(!details.working_directory.is_empty());
        assert!(details.started_at > 0);
    }

    // Its /proc files are not valid UTF-8 either. Read as text they came
    // back empty: no user, so it was listed as a system service and hidden,
    // and no start time, so a stop had only its name to go on.
    #[test]
    fn a_process_whose_name_is_not_utf8_still_has_its_details() {
        use std::os::unix::ffi::OsStrExt;

        let dir = tempfile::tempdir().unwrap();
        let odd_name = dir.path().join(std::ffi::OsStr::from_bytes(b"sl\xffeep"));
        let sleep = ["/bin/sleep", "/usr/bin/sleep"]
            .into_iter()
            .find(|path| Path::new(path).exists())
            .expect("a sleep binary");
        std::os::unix::fs::symlink(sleep, &odd_name).unwrap();
        let mut child = Command::new(&odd_name)
            .arg("60")
            .spawn()
            .expect("spawn sleep under an odd name");

        // The child is a copy of this process until its exec completes, and
        // its arguments are the last thing an exec puts in place.
        let proc_dir = Path::new("/proc").join(child.id().to_string());
        let started = || {
            fs::read(proc_dir.join("cmdline"))
                .unwrap_or_default()
                .ends_with(b"eep\x0060\x00")
        };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !started() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let renamed = started()
            && fs::read(proc_dir.join("stat"))
                .unwrap_or_default()
                .contains(&0xff);
        let details = read_details(child.id());
        let _ = child.kill();
        let _ = child.wait();

        assert!(renamed, "the child never took its odd name");
        assert!(!details.user.is_empty());
        assert!(details.started_at > 0);
        assert!(details.command_line.ends_with(" 60"), "{details:?}");
    }

    #[test]
    fn a_process_that_does_not_exist_has_no_details() {
        assert_eq!(read_details(u32::MAX), ProcessDetails::default());
    }

    #[test]
    fn scan_live() {
        let raw = scan(false).expect("scan should succeed on Linux");
        assert!(!raw.details.contains_key(&0));
    }
}
