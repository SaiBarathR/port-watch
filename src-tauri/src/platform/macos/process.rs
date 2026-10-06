//! What the kernel says about a process, asked directly. A scan used to run
//! `ps` and a second `lsof` for this on every pass.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::platform::parsers::procargs;
use crate::scanner::ProcessDetails;

/// None when any part of it could not be read: the process has exited, or it
/// is another user's. The caller asks `ps` and `lsof` about those instead.
pub fn read_details(pid: u32) -> Option<ProcessDetails> {
    let info = super::shell::bsd_info(pid).ok()?;

    Some(ProcessDetails {
        // The effective user, which is what `ps -o user=` prints.
        user: user_name(info.pbi_uid)?,
        command_line: command_line(pid)?,
        working_directory: working_directory(pid)?,
        executable_path: executable_path(pid)?,
        started_at: info.pbi_start_tvsec,
        delete_blocked: None,
    })
}

fn executable_path(pid: u32) -> Option<String> {
    let mut path = [0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    // SAFETY: `path` is a writable buffer of the size passed, and the call
    // returns how many bytes of it were written.
    let written = unsafe {
        libc::proc_pidpath(
            pid as libc::c_int,
            path.as_mut_ptr().cast(),
            path.len() as u32,
        )
    };
    let written = usize::try_from(written)
        .ok()
        .filter(|written| *written > 0)?;
    Some(String::from_utf8_lossy(path.get(..written)?).into_owned())
}

fn working_directory(pid: u32) -> Option<String> {
    let mut info = std::mem::MaybeUninit::<libc::proc_vnodepathinfo>::zeroed();
    let size = std::mem::size_of::<libc::proc_vnodepathinfo>() as libc::c_int;
    // SAFETY: `info` is a writable, zero-initialised buffer of exactly `size`
    // bytes, which is what PROC_PIDVNODEPATHINFO fills, and every bit pattern
    // is a valid `proc_vnodepathinfo`.
    let info = unsafe {
        let written = libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDVNODEPATHINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        );
        if written != size {
            return None;
        }
        info.assume_init()
    };

    // One NUL-terminated path; the libc crate types it as 32 rows of 32.
    let path: Vec<u8> = info
        .pvi_cdir
        .vip_path
        .iter()
        .flatten()
        .take_while(|&&byte| byte != 0)
        .map(|&byte| byte as u8)
        .collect();
    Some(String::from_utf8_lossy(&path).into_owned())
}

fn command_line(pid: u32) -> Option<String> {
    let mut reply = vec![0u8; argument_space()];
    let mut size = reply.len();
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as libc::c_int];
    // SAFETY: `mib` names one process, `reply` is a writable buffer of `size`
    // bytes, and the kernel writes at most `size` bytes, storing how many it
    // wrote back into `size`.
    let status = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as libc::c_uint,
            reply.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if status != 0 {
        return None;
    }
    procargs::parse_command_line(reply.get(..size)?)
}

// The most a process's arguments and environment can take up, which is the
// most KERN_PROCARGS2 can return.
fn argument_space() -> usize {
    static SPACE: OnceLock<usize> = OnceLock::new();
    *SPACE.get_or_init(|| {
        let mut space: libc::c_int = 0;
        let mut size = std::mem::size_of::<libc::c_int>();
        let mut mib = [libc::CTL_KERN, libc::KERN_ARGMAX];
        // SAFETY: `space` is a writable c_int and `size` is its size.
        let status = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(),
                mib.len() as libc::c_uint,
                (&mut space as *mut libc::c_int).cast(),
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        usize::try_from(space)
            .ok()
            .filter(|space| status == 0 && *space > 0)
            .unwrap_or(1 << 20)
    })
}

fn user_name(uid: libc::uid_t) -> Option<String> {
    static NAMES: Mutex<Option<HashMap<libc::uid_t, Option<String>>>> = Mutex::new(None);

    match NAMES.lock() {
        Ok(mut names) => names
            .get_or_insert_with(HashMap::new)
            .entry(uid)
            .or_insert_with(|| look_up_user_name(uid))
            .clone(),
        Err(_) => look_up_user_name(uid),
    }
}

fn look_up_user_name(uid: libc::uid_t) -> Option<String> {
    let mut entry = std::mem::MaybeUninit::<libc::passwd>::zeroed();
    let mut strings = [0 as libc::c_char; 4096];
    let mut found: *mut libc::passwd = std::ptr::null_mut();
    // SAFETY: getpwuid_r fills `entry` and keeps its strings in `strings`;
    // both outlive the read of `pw_name` below. `found` is left null when
    // there is no such user, and is checked before it is followed.
    unsafe {
        let status = libc::getpwuid_r(
            uid,
            entry.as_mut_ptr(),
            strings.as_mut_ptr(),
            strings.len(),
            &mut found,
        );
        if status != 0 || found.is_null() || (*found).pw_name.is_null() {
            return None;
        }
        Some(
            std::ffi::CStr::from_ptr((*found).pw_name)
                .to_string_lossy()
                .into_owned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::parsers::{lsof, ps};
    use crate::platform::unix::testing::spawn_sleep;
    use std::process::Command;

    fn stdout(command: &mut Command) -> String {
        String::from_utf8_lossy(&command.output().expect("run").stdout).into_owned()
    }

    #[test]
    fn reads_what_a_process_was_started_with() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().canonicalize().unwrap();
        let mut child = spawn_sleep(Some(&folder));
        let details = read_details(child.id());
        let _ = child.kill();
        let _ = child.wait();

        let details = details.expect("a child of this process can be read");
        assert_eq!(details.command_line, "sleep 60");
        assert_eq!(details.working_directory, folder.to_string_lossy());
        assert!(
            details.executable_path.ends_with("/sleep"),
            "{}",
            details.executable_path
        );
        assert_eq!(details.user, stdout(Command::new("id").arg("-un")).trim());
        assert!(details.started_at > 0);
        assert_eq!(details.delete_blocked, None);
    }

    // The tools a scan used to run for this are the witness.
    #[test]
    fn agrees_with_ps_and_lsof() {
        let dir = tempfile::tempdir().unwrap();
        // An empty argument and one with a space in it, to show both come
        // through the way ps prints them.
        let mut child = Command::new("perl")
            .args(["-e", "sleep 60", "", "two words"])
            .current_dir(dir.path())
            .spawn()
            .expect("spawn perl");
        let pid = child.id();
        // Until its exec completes, the child is still a copy of this test.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let details = loop {
            match read_details(pid) {
                Some(details) if details.command_line.starts_with("perl") => break details,
                _ if std::time::Instant::now() >= deadline => panic!("perl did not start"),
                _ => std::thread::sleep(std::time::Duration::from_millis(5)),
            }
        };
        let from_ps = ps::parse(&stdout(Command::new("ps").args([
            "-ww",
            "-p",
            &pid.to_string(),
            "-o",
            "pid=,user=,command=",
        ])));
        let from_lsof = lsof::parse_paths(&stdout(Command::new("lsof").args([
            "-a",
            "-p",
            &pid.to_string(),
            "-d",
            "cwd,txt",
            "-Fn",
        ])));
        let started_at = super::super::shell::process_started_at(pid);
        let _ = child.kill();
        let _ = child.wait();

        assert_eq!(details.command_line, "perl -e sleep 60  two words");
        assert_eq!(details.user, from_ps[&pid].user);
        assert_eq!(details.command_line, from_ps[&pid].command_line);
        assert_eq!(details.working_directory, from_lsof[&pid].working_directory);
        assert_eq!(details.executable_path, from_lsof[&pid].executable_path);
        assert_eq!(Some(details.started_at), started_at);
    }

    #[test]
    fn a_process_that_cannot_be_read_has_no_details() {
        // No such process.
        let mut child = Command::new("true").spawn().expect("spawn true");
        let pid = child.id();
        child.wait().expect("wait");
        assert_eq!(read_details(pid), None);

        // launchd is root's; as root it can be read after all.
        // SAFETY: geteuid has no preconditions.
        if unsafe { libc::geteuid() } != 0 {
            assert_eq!(read_details(1), None);
        }
    }

    #[test]
    fn a_uid_nobody_has_is_no_name() {
        assert_eq!(user_name(0).as_deref(), Some("root"));
        assert_eq!(user_name(4_000_000_123), None);
    }
}
