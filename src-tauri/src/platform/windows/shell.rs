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
    // Held to the end. taskkill is given a PID, and a PID is not issued
    // again while a handle to its process is open, so what is checked below
    // is what taskkill is then pointed at.
    let _held = ProcessHandle::open(pid);

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

/// When the process started, read the way a stop reads it, so that a scan
/// and the stop that follows it agree.
pub fn process_started_at(pid: u32) -> Option<u64> {
    match live_process(pid) {
        LiveProcess::Running { started_at, .. } if started_at > 0 => Some(started_at),
        _ => None,
    }
}

pub fn probe() -> Probe {
    Probe {
        live: live_process,
        names_match: crate::platform::shared::process_names_match,
        // A scan reads the start time the same way, but falls back to
        // PowerShell's when it cannot open the process, and that one may
        // land a second away.
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

mod win32 {
    use std::ffi::c_void;

    pub type Handle = *mut c_void;

    #[repr(C)]
    #[derive(Default)]
    pub struct FileTime {
        pub low: u32,
        pub high: u32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        pub fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> Handle;
        pub fn CloseHandle(handle: Handle) -> i32;
        pub fn GetLastError() -> u32;
        pub fn GetExitCodeProcess(process: Handle, exit_code: *mut u32) -> i32;
        pub fn GetProcessTimes(
            process: Handle,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
        pub fn QueryFullProcessImageNameW(
            process: Handle,
            flags: u32,
            name: *mut u16,
            size: *mut u32,
        ) -> i32;
    }

    pub const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    pub const ERROR_INVALID_PARAMETER: u32 = 87;
    pub const STILL_ACTIVE: u32 = 259;
}

// A process object, and with it the PID, outlives its process for as long as
// anyone holds a handle to it.
struct ProcessHandle(win32::Handle);

impl ProcessHandle {
    // The error is the Win32 code.
    fn open(pid: u32) -> Result<Self, u32> {
        // SAFETY: OpenProcess takes three integers. Null is its failure
        // value, and the error code is read before any other call.
        unsafe {
            let handle = win32::OpenProcess(win32::PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                Err(win32::GetLastError())
            } else {
                Ok(Self(handle))
            }
        }
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        // SAFETY: the handle came from OpenProcess and is closed only here.
        unsafe {
            win32::CloseHandle(self.0);
        }
    }
}

// Asked of the kernel directly. `tasklist` reports no start time, and a
// failure to run it could not be told from "no such process".
fn live_process(pid: u32) -> LiveProcess {
    let process = match ProcessHandle::open(pid) {
        Ok(process) => process,
        // "Invalid parameter" is how a PID that names no process is
        // reported. Any other failure (access denied, for a protected
        // process) means there is one.
        Err(win32::ERROR_INVALID_PARAMETER) => return LiveProcess::Gone,
        Err(_) => {
            return LiveProcess::Running {
                name: None,
                started_at: 0,
            }
        }
    };

    // SAFETY: plain Win32 calls on a handle that stays open until `process`
    // is dropped. Every pointer is to a live local of the type the call
    // writes, and the name buffer is passed with its capacity.
    unsafe {
        let mut exit_code = 0u32;
        if win32::GetExitCodeProcess(process.0, &mut exit_code) != 0
            && exit_code != win32::STILL_ACTIVE
        {
            return LiveProcess::Gone;
        }

        let mut creation = win32::FileTime::default();
        let (mut exit, mut kernel, mut user) = (
            win32::FileTime::default(),
            win32::FileTime::default(),
            win32::FileTime::default(),
        );
        let started_at =
            if win32::GetProcessTimes(process.0, &mut creation, &mut exit, &mut kernel, &mut user)
                != 0
            {
                crate::platform::shared::filetime_to_unix_seconds(
                    (u64::from(creation.high) << 32) | u64::from(creation.low),
                )
            } else {
                0
            };

        let mut path = [0u16; 1024];
        let mut length = path.len() as u32;
        let name =
            if win32::QueryFullProcessImageNameW(process.0, 0, path.as_mut_ptr(), &mut length) != 0
            {
                String::from_utf16_lossy(&path[..length as usize])
                    .rsplit(['\\', '/'])
                    .next()
                    .filter(|name| !name.is_empty())
                    .map(str::to_string)
            } else {
                None
            };

        LiveProcess::Running { name, started_at }
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

        // An exited process whose handle is still held, here by `child`:
        // the PID still opens, and cannot have been issued to anyone else.
        let mut child = pinger();
        let pid = child.id();
        child.kill().expect("kill");
        child.wait().expect("wait");
        assert_eq!(live_process(pid), LiveProcess::Gone);
        assert_eq!(process_started_at(pid), None);
        drop(child);
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

    // `child` keeps its handle, so the PID cannot have gone to another
    // process by the time it is looked up.
    #[test]
    fn an_exited_process_counts_as_stopped() {
        let mut child = pinger();
        let identity = identity_of(child.id());
        let pid = child.id();
        child.kill().expect("kill");
        child.wait().expect("wait");

        assert_eq!(stop_process(pid, false, Some(&identity)), Ok(()));
        drop(child);
    }

    #[test]
    fn a_scan_reads_the_start_time_the_way_a_stop_does() {
        let mut child = pinger();
        let identity = identity_of(child.id());
        let started_at = process_started_at(child.id());
        let _ = child.kill();
        let _ = child.wait();

        assert_eq!(started_at, Some(identity.started_at));
    }
}
