use std::process::Command;

use crate::platform::identity::{verdict, LiveProcess, Probe, Verdict};
use crate::scanner::ProcessIdentity;

// In a windows_subsystem = "windows" app, console children (taskkill, clip,
// tasklist, powershell) each flash a visible console window unless spawned
// with CREATE_NO_WINDOW. GUI children (explorer, wt) and the deliberately
// visible terminal must NOT get the flag.
pub(crate) trait NoWindow {
    fn no_window(&mut self) -> &mut Self;
}

impl NoWindow for Command {
    fn no_window(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        self.creation_flags(CREATE_NO_WINDOW)
    }
}

pub fn open_in_file_manager(path: &str) -> Result<(), String> {
    // explorer.exe exits with code 1 even on success, so only spawn errors
    // are treated as failures.
    Command::new("explorer")
        .arg(format!("/select,{path}"))
        .spawn()
        .map_err(|e| format!("Failed to open Explorer: {e}"))?;

    Ok(())
}

pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new("clip")
        .no_window()
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run clip: {e}"))?;

    // clip.exe interprets piped input in the OEM codepage unless it carries a
    // UTF-16LE BOM, which mangles non-ASCII paths — so send UTF-16LE.
    let mut bytes: Vec<u8> = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }

    child
        .stdin
        .as_mut()
        .ok_or_else(|| "Failed to open clip stdin".to_string())?
        .write_all(&bytes)
        .map_err(|e| format!("Failed to write to clip: {e}"))?;

    let status = child.wait().map_err(|e| format!("clip failed: {e}"))?;
    if !status.success() {
        return Err("clip exited with an error".into());
    }

    Ok(())
}

pub fn open_in_terminal(cwd: &str) -> Result<(), String> {
    if let Ok(status) = Command::new("wt").args(["-d", cwd]).status() {
        if status.success() {
            return Ok(());
        }
    }

    Command::new("cmd")
        .current_dir(cwd)
        .spawn()
        .map_err(|e| format!("Failed to open terminal: {e}"))?;

    Ok(())
}

pub fn stop_process(
    pid: u32,
    force: bool,
    expected: Option<&ProcessIdentity>,
) -> Result<(), String> {
    if !still_there(pid, expected)? {
        // Nothing is signalled: a PID that is free now can be another
        // process's a moment later.
        return Ok(());
    }

    let mut command = Command::new("taskkill");
    command.no_window().args(["/PID", &pid.to_string()]);
    if force {
        command.arg("/F");
    }

    let status = command
        .status()
        .map_err(|e| format!("Failed to run taskkill: {e}"))?;

    if !status.success() && !force {
        // Console processes reject the graceful WM_CLOSE path outright;
        // re-verify identity before force-killing.
        if !still_there(pid, expected)? {
            return Ok(());
        }
        let force_status = Command::new("taskkill")
            .no_window()
            .args(["/F", "/PID", &pid.to_string()])
            .status()
            .map_err(|e| format!("Failed to run taskkill: {e}"))?;
        if !force_status.success() {
            return Err(format!("taskkill failed for PID {pid}"));
        }
        return Ok(());
    }

    if !status.success() {
        return Err(format!("taskkill failed for PID {pid}"));
    }

    Ok(())
}

pub fn is_running(pid: u32) -> bool {
    live_process(pid) != LiveProcess::Gone
}

pub fn probe() -> Probe {
    Probe {
        live: live_process,
        names_match: crate::platform::shared::process_names_match,
        // The scan's start time comes through PowerShell and this one straight
        // from the kernel, so the two may land a second apart.
        start_slack: 1,
    }
}

// Ok(false) when the process has exited; an error when what runs under the
// PID is not, or cannot be shown to be, the listed process.
fn still_there(pid: u32, expected: Option<&ProcessIdentity>) -> Result<bool, String> {
    let now = verdict(&probe(), pid, expected);
    match (&now, expected) {
        (Verdict::Gone, _) => Ok(false),
        (Verdict::Same, _) | (_, None) => Ok(true),
        (_, Some(expected)) => Err(now
            .refusal(pid, expected)
            .unwrap_or_else(|| format!("PID {pid} could not be stopped"))),
    }
}

// Asked of the kernel directly. `tasklist` reports no start time, and a
// failure to run it could not be told from "no such process".
fn live_process(pid: u32) -> LiveProcess {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> *mut c_void;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn GetLastError() -> u32;
        fn GetExitCodeProcess(process: *mut c_void, exit_code: *mut u32) -> i32;
        fn GetProcessTimes(
            process: *mut c_void,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
        fn QueryFullProcessImageNameW(
            process: *mut c_void,
            flags: u32,
            name: *mut u16,
            size: *mut u32,
        ) -> i32;
    }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const ERROR_INVALID_PARAMETER: u32 = 87;
    const STILL_ACTIVE: u32 = 259;

    // SAFETY: plain Win32 calls. Every pointer is to a live local of the type
    // the call writes, the name buffer is passed with its capacity, and the
    // handle is closed before returning.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            // "Invalid parameter" is how a PID that names no process is
            // reported. Any other failure (access denied, for a protected
            // process) means there is one.
            return if GetLastError() == ERROR_INVALID_PARAMETER {
                LiveProcess::Gone
            } else {
                LiveProcess::Running {
                    name: None,
                    started_at: 0,
                }
            };
        }

        // A process object outlives its process for as long as anyone holds
        // a handle to it.
        let mut exit_code = 0u32;
        let exited = GetExitCodeProcess(handle, &mut exit_code) != 0 && exit_code != STILL_ACTIVE;

        let mut creation = FileTime::default();
        let (mut exit, mut kernel, mut user) = (
            FileTime::default(),
            FileTime::default(),
            FileTime::default(),
        );
        let started_at =
            if GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) != 0 {
                crate::platform::shared::filetime_to_unix_seconds(
                    (u64::from(creation.high) << 32) | u64::from(creation.low),
                )
            } else {
                0
            };

        let mut path = [0u16; 1024];
        let mut length = path.len() as u32;
        let name = if QueryFullProcessImageNameW(handle, 0, path.as_mut_ptr(), &mut length) != 0 {
            String::from_utf16_lossy(&path[..length as usize])
                .rsplit(['\\', '/'])
                .next()
                .filter(|name| !name.is_empty())
                .map(str::to_string)
        } else {
            None
        };

        CloseHandle(handle);

        if exited {
            LiveProcess::Gone
        } else {
            LiveProcess::Running { name, started_at }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Stdio;

    fn unix_now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    // A console program that stays alive on its own, with no children.
    fn pinger() -> std::process::Child {
        Command::new("ping")
            .args(["-n", "60", "127.0.0.1"])
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn ping")
    }

    fn identity_of(pid: u32) -> ProcessIdentity {
        match live_process(pid) {
            LiveProcess::Running {
                name: Some(name),
                started_at,
            } => ProcessIdentity { name, started_at },
            other => panic!("PID {pid} should be running and readable: {other:?}"),
        }
    }

    #[test]
    fn this_process_is_running_under_its_own_name_and_start_time() {
        let pid = std::process::id();
        let LiveProcess::Running { name, started_at } = live_process(pid) else {
            panic!("the test process should be running");
        };
        let exe = std::env::current_exe().unwrap();
        let exe_name = exe.file_name().unwrap().to_string_lossy();

        assert!(
            (probe().names_match)(name.as_deref().unwrap(), &exe_name),
            "{name:?} vs {exe_name}"
        );
        let now = unix_now();
        assert!(
            started_at > 0 && started_at <= now + 1,
            "{started_at} vs {now}"
        );
        assert!(now - started_at.min(now) < 3_600, "{started_at} vs {now}");
        assert!(is_running(pid));
    }

    #[test]
    fn a_pid_that_names_no_process_is_gone() {
        // PIDs are multiples of four, so this one is never issued.
        assert_eq!(live_process(u32::MAX - 2), LiveProcess::Gone);
        assert!(!is_running(u32::MAX - 2));

        let mut child = pinger();
        let pid = child.id();
        child.kill().expect("kill");
        child.wait().expect("wait");
        drop(child);
        assert_eq!(live_process(pid), LiveProcess::Gone);
    }

    #[test]
    fn stops_the_process_it_was_given() {
        for force in [true, false] {
            let mut child = pinger();
            let identity = identity_of(child.id());
            assert_eq!(identity.name.to_ascii_lowercase(), "ping.exe");

            let result = stop_process(child.id(), force, Some(&identity));
            let status = child.wait().expect("wait");

            assert_eq!(result, Ok(()), "force={force}");
            assert!(
                !status.success(),
                "force={force}: it should have been killed"
            );
        }
    }

    // The same program under the same PID, started at another time.
    #[test]
    fn refuses_a_pid_reused_by_the_same_program() {
        let mut child = pinger();
        let live = identity_of(child.id());
        let earlier = ProcessIdentity {
            started_at: live.started_at - 60,
            ..live
        };

        let result = stop_process(child.id(), true, Some(&earlier));
        let still_running = child.try_wait().expect("try_wait").is_none();
        let _ = child.kill();
        let _ = child.wait();

        assert!(result.unwrap_err().contains("a newer"));
        assert!(still_running, "a reused PID must not be signalled");
    }

    #[test]
    fn refuses_a_pid_that_now_runs_a_different_program() {
        let mut child = pinger();
        let expected = ProcessIdentity {
            name: "node.exe".into(),
            ..identity_of(child.id())
        };

        let result = stop_process(child.id(), true, Some(&expected));
        let still_running = child.try_wait().expect("try_wait").is_none();
        let _ = child.kill();
        let _ = child.wait();

        assert!(result.unwrap_err().contains("now belongs to"));
        assert!(still_running);
    }

    #[test]
    fn an_exited_process_counts_as_stopped_without_running_taskkill() {
        let mut child = pinger();
        let identity = identity_of(child.id());
        let pid = child.id();
        child.kill().expect("kill");
        child.wait().expect("wait");
        drop(child);

        assert_eq!(stop_process(pid, false, Some(&identity)), Ok(()));
    }
}
