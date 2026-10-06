pub fn open_in_file_manager(path: &str) -> Result<(), String> {
    let mut command = std::process::Command::new("open");
    // `open` launches a bundle (Foo.app) rather than showing what is in it,
    // and any folder with an extension may be one: reveal those in their
    // parent folder instead.
    if std::path::Path::new(path).extension().is_some() {
        command.arg("-R");
    }
    let status = command
        .arg(path)
        .status()
        .map_err(|e| format!("Failed to open Finder: {e}"))?;

    if !status.success() {
        return Err(format!("open command failed for: {path}"));
    }

    Ok(())
}

pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run pbcopy: {e}"))?;

    child
        .stdin
        .as_mut()
        .ok_or_else(|| "Failed to open pbcopy stdin".to_string())?
        .write_all(text.as_bytes())
        .map_err(|e| format!("Failed to write to pbcopy: {e}"))?;

    let status = child.wait().map_err(|e| format!("pbcopy failed: {e}"))?;

    if !status.success() {
        return Err("pbcopy exited with an error".into());
    }

    Ok(())
}

pub fn open_in_terminal(cwd: &str) -> Result<(), String> {
    let status = std::process::Command::new("open")
        .args(["-a", "Terminal", cwd])
        .status()
        .map_err(|e| format!("Failed to open Terminal: {e}"))?;

    if !status.success() {
        return Err(format!("Failed to open Terminal at: {cwd}"));
    }

    Ok(())
}

// SIGTERM gets this long before escalating to SIGKILL, and SIGKILL gets as
// long again to take effect before the stop is reported as failed.
const STOP_GRACE: std::time::Duration = std::time::Duration::from_secs(2);
const EXIT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

pub fn stop_process(pid: u32, force: bool, expected_name: Option<&str>) -> Result<(), String> {
    verify_process_identity(pid, expected_name)?;

    if !force {
        send_signal(pid, "-TERM")?;
        // The wait re-verifies identity on every poll, so reaching the
        // escalation below means the PID was not reused by an unrelated
        // process during the grace window.
        if wait_for_exit(pid, expected_name, STOP_GRACE) {
            return Ok(());
        }
    }

    send_signal(pid, "-KILL")?;
    if wait_for_exit(pid, expected_name, STOP_GRACE) {
        Ok(())
    } else {
        Err(format!("PID {pid} is still running after SIGKILL"))
    }
}

// The kernel's name for a live process: the `pbi_name` field lsof prints as
// the command name during a scan, so an unchanged process compares equal to
// what the user saw. `ps` cannot supply it: its `comm` is argv[0], which
// launchers and processes rewrite (`python3` for python3.13, Next.js's
// `next-server (v…)`, puma, nginx workers), and its `ucomm` keeps only 16
// bytes, which sibling helpers share (`Google Chrome He…`).
pub fn current_process_name(pid: u32) -> Option<String> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY: `info` is a writable, zero-initialised buffer of exactly `size`
    // bytes, which is what PROC_PIDTBSDINFO fills, and every bit pattern is
    // a valid `proc_bsdinfo`.
    let info = unsafe {
        let written = libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        );
        if written != size {
            return None;
        }
        info.assume_init()
    };
    // A zombie has already exited and released its ports; it only lingers
    // until its parent reaps it.
    if info.pbi_status == libc::SZOMB {
        return None;
    }

    let raw = if info.pbi_name[0] != 0 {
        &info.pbi_name[..]
    } else {
        &info.pbi_comm[..]
    };
    let bytes: Vec<u8> = raw
        .iter()
        .take_while(|&&byte| byte != 0)
        .map(|&byte| byte as u8)
        .collect();
    if bytes.is_empty() {
        None
    } else {
        Some(String::from_utf8_lossy(&bytes).into_owned())
    }
}

/// When the process started, in Unix seconds. Unlike `ps -o etime`, it does
/// not move from one scan to the next. `KERN_PROC_PID` answers for any user's
/// process; `proc_pidinfo` refuses the ones this user does not own.
pub fn process_started_at(pid: u32) -> Option<u64> {
    // The reply is a `struct kinfo_proc` (<sys/sysctl.h>), which the libc
    // crate does not carry. It opens with the process's start time as a
    // `timeval`, and that is all that is read from it; the buffer is simply
    // larger than the struct's 648 bytes.
    #[repr(C, align(8))]
    struct Reply([u8; 1024]);

    let mut reply = Reply([0; 1024]);
    let mut size = std::mem::size_of::<Reply>();
    let mut mib = [
        libc::CTL_KERN,
        libc::KERN_PROC,
        libc::KERN_PROC_PID,
        pid as libc::c_int,
    ];
    // SAFETY: `mib` names one process, `reply` is a writable buffer of `size`
    // bytes, and the kernel writes at most `size` bytes, storing how many it
    // wrote back into `size`.
    let status = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as libc::c_uint,
            reply.0.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    // A PID that does not exist is a success with nothing written.
    if status != 0 || size < std::mem::size_of::<libc::timeval>() {
        return None;
    }

    let seconds = i64::from_ne_bytes(reply.0[..8].try_into().ok()?);
    u64::try_from(seconds).ok().filter(|seconds| *seconds > 0)
}

// Exact comparison: both names are the kernel's, and anything looser would
// let one sibling helper pass for another.
fn verify_process_identity(pid: u32, expected_name: Option<&str>) -> Result<(), String> {
    let Some(expected) = expected_name else {
        return Ok(());
    };

    match current_process_name(pid) {
        Some(name) if name == expected => Ok(()),
        Some(name) => Err(format!(
            "PID {pid} now belongs to \"{name}\", not \"{expected}\" — the process list was stale. Refresh and try again."
        )),
        // Already gone: the goal (process stopped) is achieved.
        None => Ok(()),
    }
}

fn still_matches(pid: u32, expected_name: Option<&str>) -> bool {
    match (current_process_name(pid), expected_name) {
        (Some(name), Some(expected)) => name == expected,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

// Polls instead of sleeping a fixed interval: most processes exit within
// milliseconds of a signal.
fn wait_for_exit(pid: u32, expected_name: Option<&str>, timeout: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if !still_matches(pid, expected_name) {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(EXIT_POLL_INTERVAL);
    }
}

fn send_signal(pid: u32, signal: &str) -> Result<(), String> {
    let output = std::process::Command::new("kill")
        .arg(signal)
        .arg(pid.to_string())
        .output()
        .map_err(|e| format!("Failed to run kill: {e}"))?;
    if output.status.success() {
        return Ok(());
    }

    // `kill: 123: Operation not permitted` -> `Operation not permitted`
    let stderr = String::from_utf8_lossy(&output.stderr);
    match stderr.trim().rsplit(": ").next().unwrap_or_default() {
        // Exited on its own before the signal landed, so it is stopped all
        // the same — common when its parent was stopped a moment earlier.
        "No such process" => Ok(()),
        "" => Err(format!("kill {signal} failed for PID {pid}")),
        reason => Err(format!("kill {signal} failed for PID {pid}: {reason}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    // What lsof calls this PID: an independent witness for the name a scan
    // reports.
    fn scanned_name(pid: u32) -> String {
        let output = Command::new("lsof")
            .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fc"])
            .output()
            .expect("lsof should run");
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .find_map(|line| line.strip_prefix('c').map(str::to_string))
            .expect("lsof should report a command name")
    }

    fn exited(child: &mut Child) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if child.try_wait().expect("try_wait").is_some() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    fn unix_now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    #[test]
    fn start_time_is_stable_and_recent_for_this_process() {
        let pid = std::process::id();
        let now = unix_now();

        let first = process_started_at(pid).expect("own start time");
        let second = process_started_at(pid).expect("own start time");

        assert_eq!(first, second);
        assert!(first <= now, "{first} is after {now}");
        assert!(now - first < 3_600, "{first} is long before {now}");
    }

    #[test]
    fn start_time_is_readable_for_another_users_process() {
        // launchd runs as root, and the scan lists such processes too.
        let launchd = process_started_at(1).expect("launchd's start time");
        assert!(launchd > 0 && launchd <= unix_now());
    }

    #[test]
    fn start_time_is_none_for_a_pid_that_does_not_exist() {
        let mut child = Command::new("true").spawn().expect("spawn true");
        let pid = child.id();
        child.wait().expect("wait");
        assert_eq!(process_started_at(pid), None);
    }

    #[test]
    fn start_time_matches_what_ps_reports() {
        let pid = std::process::id();
        let output = Command::new("ps")
            .env("LC_ALL", "C")
            .args(["-p", &pid.to_string(), "-o", "etime="])
            .output()
            .expect("ps should run");
        // etime is [[dd-]hh:]mm:ss; a test process is seconds old.
        let etime = String::from_utf8_lossy(&output.stdout);
        let seconds: u64 = etime
            .trim()
            .rsplit(':')
            .zip([1, 60, 3_600])
            .map(|(part, unit)| part.parse::<u64>().unwrap() * unit)
            .sum();

        let started = process_started_at(pid).expect("own start time");
        let from_ps = unix_now() - seconds;
        assert!(started.abs_diff(from_ps) <= 2, "{started} vs {from_ps}");
    }

    #[test]
    fn identity_uses_the_full_executable_name() {
        // This test binary (`port_watch_lib-<hash>`) has a name longer than
        // the 16 bytes `ps -o ucomm` keeps, like most app helper processes.
        let pid = std::process::id();
        let name = scanned_name(pid);
        assert!(name.len() > 16, "{name}");
        assert_eq!(current_process_name(pid), Some(name.clone()));
        assert_eq!(verify_process_identity(pid, Some(&name)), Ok(()));

        // Chrome's helpers and WebKit's XPC services differ only past the
        // 16th byte; one must not pass for another.
        let sibling = &name[..name.len() - 1];
        assert!(verify_process_identity(pid, Some(sibling)).is_err());
    }

    #[test]
    fn stops_process_whose_argv0_differs_from_its_executable_name() {
        // Next.js, puma, nginx workers and `python3` -> `python3.13` all show
        // an argv[0] that is not the executable name the scan reports.
        for force in [true, false] {
            let mut child = Command::new("sleep")
                .arg("60")
                .arg0("next-server (v15.3.2)")
                .spawn()
                .expect("spawn sleep");
            let name = scanned_name(child.id());
            assert_eq!(name, "sleep");

            let result = stop_process(child.id(), force, Some(&name));
            let stopped = exited(&mut child);
            let _ = child.kill();
            assert_eq!(result, Ok(()), "force={force}");
            assert!(stopped, "force={force}: process should be gone");
        }
    }

    #[test]
    fn refuses_pid_that_now_runs_a_different_program() {
        let mut child = Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("spawn sleep");

        let result = stop_process(child.id(), true, Some("node"));
        let still_running = child.try_wait().expect("try_wait").is_none();
        let _ = child.kill();
        let _ = child.wait();

        assert!(result.unwrap_err().contains("now belongs to"));
        assert!(still_running, "a mismatched PID must not be signalled");
    }

    #[test]
    fn already_exited_process_counts_as_stopped() {
        let mut child = Command::new("true").spawn().expect("spawn true");
        let pid = child.id();
        child.wait().expect("wait");

        assert_eq!(stop_process(pid, false, Some("true")), Ok(()));
        assert_eq!(stop_process(pid, true, None), Ok(()));
    }

    #[test]
    fn every_scanned_process_passes_its_own_identity_check() {
        let scanned = crate::scanner::scan_listening_ports(false).expect("scan");
        for process in scanned {
            // None: it exited between the scan and this lookup.
            if let Some(name) = current_process_name(process.pid) {
                assert_eq!(name, process.name, "PID {}", process.pid);
            }
        }
    }

    #[test]
    fn failing_to_signal_a_live_process_is_an_error() {
        // SAFETY: geteuid has no preconditions.
        if unsafe { libc::geteuid() } == 0 {
            return; // root may signal anything
        }
        // Signal 0 only probes permission; launchd is never really signalled.
        let error = send_signal(1, "-0").unwrap_err();
        assert!(error.contains("not permitted"), "{error}");
    }

    #[test]
    fn unreaped_zombie_counts_as_gone() {
        let mut child = Command::new("true").spawn().expect("spawn true");
        let deadline = Instant::now() + Duration::from_secs(5);
        while current_process_name(child.id()).is_some() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let name = current_process_name(child.id());
        let _ = child.wait();
        assert_eq!(name, None);
    }

    #[test]
    fn escalates_to_sigkill_when_sigterm_is_ignored() {
        let mut child = Command::new("perl")
            .args([
                "-e",
                "$SIG{TERM} = 'IGNORE'; $| = 1; print \"ready\\n\"; sleep 60",
            ])
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn perl");
        let mut ready = String::new();
        BufReader::new(child.stdout.take().expect("stdout"))
            .read_line(&mut ready)
            .expect("read ready line");
        let name = scanned_name(child.id());

        let started = Instant::now();
        let result = stop_process(child.id(), false, Some(&name));
        let elapsed = started.elapsed();
        let stopped = exited(&mut child);
        let _ = child.kill();

        assert_eq!(result, Ok(()));
        assert!(stopped, "process should be gone after escalation");
        assert!(elapsed >= STOP_GRACE, "SIGTERM grace was skipped");
    }

    #[test]
    fn graceful_stop_returns_as_soon_as_the_process_exits() {
        let mut child = Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("spawn sleep");

        let started = Instant::now();
        let result = stop_process(child.id(), false, Some("sleep"));
        let elapsed = started.elapsed();
        let stopped = exited(&mut child);
        let _ = child.kill();

        assert_eq!(result, Ok(()));
        assert!(stopped);
        assert!(elapsed < Duration::from_secs(1), "took {elapsed:?}");
    }
}
