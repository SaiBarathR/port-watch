use crate::platform::identity::{LiveProcess, Probe};
use crate::platform::unix;
use crate::scanner::ProcessIdentity;

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

pub fn stop_process(
    pid: u32,
    force: bool,
    expected: Option<&ProcessIdentity>,
) -> Result<(), String> {
    unix::stop_process(&probe(), pid, force, expected)
}

pub fn is_running(pid: u32) -> bool {
    live_process(pid) != LiveProcess::Gone
}

// Names are compared exactly: both are the kernel's, and anything looser
// would let one sibling helper pass for another.
pub fn probe() -> Probe {
    Probe {
        live: live_process,
        names_match: |live, scanned| live == scanned,
        // The kernel keeps one start time per process, whichever call
        // reports it.
        start_slack: 0,
    }
}

fn live_process(pid: u32) -> LiveProcess {
    match bsd_info(pid) {
        Ok(info) if info.pbi_status == libc::SZOMB => LiveProcess::Gone,
        // Name and start time from one reply, so they describe one process
        // and not two that held the PID in turn.
        Ok(info) => LiveProcess::Running {
            name: process_name(&info),
            started_at: info.pbi_start_tvsec,
        },
        // No such process, or one that has exited and waits to be reaped.
        // Only ESRCH says it is gone.
        Err(error) if error.raw_os_error() == Some(libc::ESRCH) => LiveProcess::Gone,
        // Anything else, such as another user's process (EPERM): it exists,
        // and its name is not ours to read. Its start time is public.
        Err(_) => match process_started_at(pid) {
            Some(started_at) => LiveProcess::Running {
                name: None,
                started_at,
            },
            None => LiveProcess::Gone,
        },
    }
}

fn process_name(info: &libc::proc_bsdinfo) -> Option<String> {
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

// Fails with EPERM for a process another user owns, and with ESRCH for one
// that has exited, zombie or not.
fn bsd_info(pid: u32) -> std::io::Result<libc::proc_bsdinfo> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY: `info` is a writable, zero-initialised buffer of exactly `size`
    // bytes, which is what PROC_PIDTBSDINFO fills, and every bit pattern is
    // a valid `proc_bsdinfo`.
    unsafe {
        let written = libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        );
        if written == size {
            Ok(info.assume_init())
        } else if written <= 0 {
            Err(std::io::Error::last_os_error())
        } else {
            // A short reply sets no error code; whatever is in errno is stale.
            Err(std::io::Error::other("short reply from proc_pidinfo"))
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command};
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

    fn name_of(pid: u32) -> Option<String> {
        match live_process(pid) {
            LiveProcess::Running { name, .. } => name,
            LiveProcess::Gone => None,
        }
    }

    fn identity(pid: u32, name: &str) -> ProcessIdentity {
        ProcessIdentity {
            name: name.to_string(),
            started_at: process_started_at(pid).expect("start time"),
        }
    }

    #[test]
    fn identity_uses_the_full_executable_name() {
        // This test binary (`port_watch_lib-<hash>`) has a name longer than
        // the 16 bytes `ps -o ucomm` keeps, like most app helper processes.
        let pid = std::process::id();
        let name = scanned_name(pid);
        assert!(name.len() > 16, "{name}");
        assert_eq!(name_of(pid), Some(name.clone()));

        // Chrome's helpers and WebKit's XPC services differ only past the
        // 16th byte; one must not pass for another.
        let sibling = &name[..name.len() - 1];
        assert!((probe().names_match)(&name, &name));
        assert!(!(probe().names_match)(&name, sibling));
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

            let expected = identity(child.id(), &name);
            let result = stop_process(child.id(), force, Some(&expected));
            let stopped = exited(&mut child);
            let _ = child.kill();
            assert_eq!(result, Ok(()), "force={force}");
            assert!(stopped, "force={force}: process should be gone");
        }
    }

    #[test]
    fn every_scanned_process_passes_its_own_identity_check() {
        let scanned = crate::scanner::scan_listening_ports(false).expect("scan");
        for process in scanned {
            match live_process(process.pid) {
                // It exited between the scan and this lookup.
                LiveProcess::Gone => {}
                LiveProcess::Running { name, started_at } => {
                    // The name is unreadable for another user's process.
                    if let Some(name) = name {
                        assert_eq!(name, process.name, "PID {}", process.pid);
                    }
                    assert_eq!(started_at, process.started_at, "PID {}", process.pid);
                }
            }
        }
    }

    // launchd runs as root: it exists and has a start time, but an ordinary
    // user cannot read its name. That must not make it look gone.
    #[test]
    fn another_users_process_is_running_not_gone() {
        // SAFETY: geteuid has no preconditions.
        if unsafe { libc::geteuid() } == 0 {
            return; // root can read every name
        }
        assert!(is_running(1));
        assert!(matches!(
            live_process(1),
            LiveProcess::Running { name: None, started_at } if started_at > 0
        ));
    }

    // The kernel still answers for a zombie's start time, and refuses its
    // details with the same call that refuses another user's process. Only
    // the error tells the two apart.
    #[test]
    fn an_unreaped_zombie_is_gone_not_unreadable() {
        let mut child = Command::new("true").spawn().expect("spawn true");
        let deadline = Instant::now() + Duration::from_secs(5);
        while live_process(child.id()) != LiveProcess::Gone && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let zombie = live_process(child.id());
        let still_has_a_start_time = process_started_at(child.id()).is_some();
        let _ = child.wait();

        assert_eq!(zombie, LiveProcess::Gone);
        assert!(still_has_a_start_time, "the child should not be reaped yet");
    }

    #[test]
    fn a_pid_that_does_not_exist_is_gone() {
        let mut child = Command::new("true").spawn().expect("spawn true");
        let pid = child.id();
        child.wait().expect("wait");
        assert_eq!(live_process(pid), LiveProcess::Gone);
        assert!(!is_running(pid));
    }
}
