pub fn open_in_file_manager(path: &str) -> Result<(), String> {
    let status = std::process::Command::new("open")
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

    let status = child
        .wait()
        .map_err(|e| format!("pbcopy failed: {e}"))?;

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

pub fn current_process_name(pid: u32) -> Option<String> {
    // `ucomm` is the kernel's name for the running executable, which is what
    // lsof reports during a scan. `comm` is argv[0], and launchers and
    // processes rewrite that freely (`python3` for python3.13, Next.js's
    // `next-server (v…)`, puma, nginx workers), so it cannot be compared
    // against the scanned name.
    let output = std::process::Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "stat=,ucomm="])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let (state, name) = stdout.trim().split_once(char::is_whitespace)?;
    // A zombie has already exited and released its ports; it only lingers
    // until its parent reaps it.
    if state.starts_with('Z') {
        return None;
    }
    let name = name.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn verify_process_identity(pid: u32, expected_name: Option<&str>) -> Result<(), String> {
    let Some(expected) = expected_name else {
        return Ok(());
    };

    match current_process_name(pid) {
        Some(name) if crate::platform::shared::process_names_match(&name, expected) => Ok(()),
        Some(name) => Err(format!(
            "PID {pid} now belongs to \"{name}\", not \"{expected}\" — the process list was stale. Refresh and try again."
        )),
        // Already gone: the goal (process stopped) is achieved.
        None => Ok(()),
    }
}

fn still_matches(pid: u32, expected_name: Option<&str>) -> bool {
    match (current_process_name(pid), expected_name) {
        (Some(name), Some(expected)) => {
            crate::platform::shared::process_names_match(&name, expected)
        }
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

    // A process that exited on its own before the signal landed is stopped
    // all the same — common when its parent was stopped a moment earlier.
    if output.status.success() || current_process_name(pid).is_none() {
        return Ok(());
    }

    // `kill: 123: Operation not permitted` -> `Operation not permitted`
    let stderr = String::from_utf8_lossy(&output.stderr);
    match stderr.trim().rsplit(": ").next() {
        Some(reason) if !reason.is_empty() => {
            Err(format!("kill {signal} failed for PID {pid}: {reason}"))
        }
        _ => Err(format!("kill {signal} failed for PID {pid}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    // The name the scanner would have shown for this PID: lsof's `c` field.
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
