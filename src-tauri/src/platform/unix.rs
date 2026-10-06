//! Stopping a process on macOS and Linux. The two differ only in how they
//! learn what a PID currently is.

use std::time::{Duration, Instant};

use crate::platform::identity::{verdict, Probe, Verdict};
use crate::scanner::ProcessIdentity;

// SIGTERM gets this long before escalating to SIGKILL, and SIGKILL gets as
// long again to take effect before the stop is reported as failed.
const STOP_GRACE: Duration = Duration::from_secs(2);
const EXIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub fn stop_process(
    probe: &Probe,
    pid: u32,
    force: bool,
    expected: Option<&ProcessIdentity>,
) -> Result<(), String> {
    match verdict(probe, pid, expected) {
        // Nothing is signalled: a PID that is free now can be another
        // process's a moment later.
        Verdict::Gone => return Ok(()),
        Verdict::Same => {}
        other => return Err(refusal(&other, pid, expected)),
    }

    if !force {
        send_signal(pid, libc::SIGTERM, "TERM")?;
        match wait_for_exit(probe, pid, expected, STOP_GRACE) {
            Wait::Exited => return Ok(()),
            // Escalating means signalling again, which needs the same
            // certainty the first signal had.
            Wait::Unconfirmed(why) => return Err(why),
            Wait::StillRunning => {}
        }
    }

    send_signal(pid, libc::SIGKILL, "KILL")?;
    match wait_for_exit(probe, pid, expected, STOP_GRACE) {
        Wait::Exited => Ok(()),
        Wait::StillRunning => Err(format!("PID {pid} is still running after SIGKILL")),
        Wait::Unconfirmed(why) => Err(why),
    }
}

fn refusal(verdict: &Verdict, pid: u32, expected: Option<&ProcessIdentity>) -> String {
    expected
        .and_then(|expected| verdict.refusal(pid, expected))
        .unwrap_or_else(|| format!("PID {pid} could not be stopped"))
}

enum Wait {
    Exited,
    StillRunning,
    /// The process could no longer be told apart from another; says why.
    Unconfirmed(String),
}

// Polls instead of sleeping a fixed interval: most processes exit within
// milliseconds of a signal.
//
// Exited means gone, or provably replaced by another process. The same
// process under a new name has not exited: a server may rename itself while
// it shuts down, and it still holds its port.
fn wait_for_exit(
    probe: &Probe,
    pid: u32,
    expected: Option<&ProcessIdentity>,
    timeout: Duration,
) -> Wait {
    let deadline = Instant::now() + timeout;
    loop {
        let now = verdict(probe, pid, expected);
        let unconfirmed = match &now {
            Verdict::Gone | Verdict::Other { certain: true, .. } => return Wait::Exited,
            Verdict::Same | Verdict::Renamed(_) => None,
            Verdict::Other { certain: false, .. } | Verdict::Unconfirmed(_) => {
                Some(refusal(&now, pid, expected))
            }
        };
        if Instant::now() >= deadline {
            return match unconfirmed {
                Some(why) => Wait::Unconfirmed(why),
                None => Wait::StillRunning,
            };
        }
        std::thread::sleep(EXIT_POLL_INTERVAL);
    }
}

fn send_signal(pid: u32, signal: libc::c_int, name: &str) -> Result<(), String> {
    // To kill(2), 0 means this process's whole group and a negative number
    // means a group or every process the user owns.
    let target = libc::pid_t::try_from(pid)
        .ok()
        .filter(|target| *target > 0)
        .ok_or_else(|| format!("Invalid PID {pid}"))?;

    // SAFETY: kill takes two integers and has no memory-safety preconditions.
    if unsafe { libc::kill(target, signal) } == 0 {
        return Ok(());
    }

    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        // Exited on its own before the signal landed, so it is stopped all
        // the same — common when its parent was stopped a moment earlier.
        Some(libc::ESRCH) => Ok(()),
        Some(libc::EPERM) => Err(format!(
            "kill -{name} failed for PID {pid}: Operation not permitted"
        )),
        _ => Err(format!("kill -{name} failed for PID {pid}: {error}")),
    }
}

#[cfg(test)]
pub mod testing {
    use crate::platform::identity::LiveProcess;
    use std::path::Path;
    use std::process::{Child, Command};
    use std::time::{Duration, Instant};

    /// Spawns `sleep 60` and returns once it runs under that name. On Linux
    /// a child still carries the name of the thread that spawned it for an
    /// instant after `spawn` returns, which would make an identity read
    /// straight away the wrong one.
    pub fn spawn_sleep(dir: Option<&Path>) -> Child {
        let mut command = Command::new("sleep");
        command.arg("60");
        if let Some(dir) = dir {
            command.current_dir(dir);
        }
        let child = command.spawn().expect("spawn sleep");

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match (crate::platform::shell::probe().live)(child.id()) {
                LiveProcess::Running {
                    name: Some(name), ..
                } if name == "sleep" => return child,
                _ if Instant::now() >= deadline => panic!("sleep did not start"),
                _ => std::thread::sleep(Duration::from_millis(5)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::identity::LiveProcess;
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};

    fn real_probe() -> Probe {
        crate::platform::shell::probe()
    }

    fn identity_of(pid: u32) -> ProcessIdentity {
        match (real_probe().live)(pid) {
            LiveProcess::Running {
                name: Some(name),
                started_at,
            } => ProcessIdentity { name, started_at },
            other => panic!("PID {pid} should be running and readable: {other:?}"),
        }
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

    fn sleeper() -> Child {
        testing::spawn_sleep(None)
    }

    #[test]
    fn graceful_stop_returns_as_soon_as_the_process_exits() {
        let mut child = sleeper();
        let identity = identity_of(child.id());

        let started = Instant::now();
        let result = stop_process(&real_probe(), child.id(), false, Some(&identity));
        let elapsed = started.elapsed();
        let stopped = exited(&mut child);
        let _ = child.kill();

        assert_eq!(result, Ok(()));
        assert!(stopped);
        assert!(elapsed < Duration::from_secs(1), "took {elapsed:?}");
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
        let identity = identity_of(child.id());

        let started = Instant::now();
        let result = stop_process(&real_probe(), child.id(), false, Some(&identity));
        let elapsed = started.elapsed();
        let stopped = exited(&mut child);
        let _ = child.kill();

        assert_eq!(result, Ok(()));
        assert!(stopped, "process should be gone after escalation");
        assert!(elapsed >= STOP_GRACE, "SIGTERM grace was skipped");
    }

    #[test]
    fn refuses_a_pid_that_now_runs_a_different_program() {
        let mut child = sleeper();
        let expected = ProcessIdentity {
            name: "node".into(),
            ..identity_of(child.id())
        };

        let result = stop_process(&real_probe(), child.id(), true, Some(&expected));
        let still_running = child.try_wait().expect("try_wait").is_none();
        let _ = child.kill();
        let _ = child.wait();

        assert!(result.unwrap_err().contains("now belongs to \"sleep\""));
        assert!(still_running, "a mismatched PID must not be signalled");
    }

    // The same program under the same PID, started later: the name matches
    // and only the start time shows it is not the process the user chose.
    #[test]
    fn refuses_a_pid_reused_by_the_same_program() {
        let mut child = sleeper();
        let live = identity_of(child.id());
        let earlier = ProcessIdentity {
            started_at: live.started_at - 60,
            ..live
        };

        let result = stop_process(&real_probe(), child.id(), true, Some(&earlier));
        let still_running = child.try_wait().expect("try_wait").is_none();
        let _ = child.kill();
        let _ = child.wait();

        assert!(result.unwrap_err().contains("a newer \"sleep\""));
        assert!(still_running, "a reused PID must not be signalled");
    }

    #[test]
    fn an_unknown_start_time_falls_back_to_the_name() {
        let mut child = sleeper();
        let name_only = ProcessIdentity {
            started_at: 0,
            ..identity_of(child.id())
        };

        let result = stop_process(&real_probe(), child.id(), true, Some(&name_only));
        let stopped = exited(&mut child);
        let _ = child.kill();

        assert_eq!(result, Ok(()));
        assert!(stopped);
    }

    #[test]
    fn already_exited_process_counts_as_stopped() {
        let mut child = Command::new("true").spawn().expect("spawn true");
        let pid = child.id();
        child.wait().expect("wait");
        let identity = ProcessIdentity {
            name: "true".into(),
            started_at: 0,
        };

        assert_eq!(
            stop_process(&real_probe(), pid, false, Some(&identity)),
            Ok(())
        );
        assert_eq!(stop_process(&real_probe(), pid, true, None), Ok(()));
    }

    #[test]
    fn unreaped_zombie_counts_as_gone() {
        let mut child = Command::new("true").spawn().expect("spawn true");
        let deadline = Instant::now() + Duration::from_secs(5);
        while (real_probe().live)(child.id()) != LiveProcess::Gone && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let live = (real_probe().live)(child.id());
        let _ = child.wait();
        assert_eq!(live, LiveProcess::Gone);
    }

    // A PID that is gone at the check is not signalled at all. Here the probe
    // reports "gone" for a process that is in fact running, which shows no
    // signal follows that answer.
    #[test]
    fn a_pid_found_gone_is_never_signalled() {
        let mut child = sleeper();
        let probe = Probe {
            live: |_| LiveProcess::Gone,
            names_match: |a, b| a == b,
            start_slack: 0,
        };

        let result = stop_process(&probe, child.id(), true, None);
        let still_running = child.try_wait().expect("try_wait").is_none();
        let _ = child.kill();
        let _ = child.wait();

        assert_eq!(result, Ok(()));
        assert!(still_running);
    }

    // A server that renames itself when told to stop (Node's `process.title`
    // changes the kernel's name on Linux) has not exited. It used to count
    // as stopped the moment its name changed, and a delete then went ahead.
    #[test]
    fn a_process_that_renames_itself_is_not_taken_for_stopped() {
        let mut child = sleeper();
        let identity = identity_of(child.id());
        // What the real probe reports, with another name once signalled:
        // SIGTERM kills `sleep` at once, so the rename is played by the probe.
        fn renamed(pid: u32) -> LiveProcess {
            match (real_probe().live)(pid) {
                LiveProcess::Running { started_at, .. } => LiveProcess::Running {
                    name: Some("shutting-down".into()),
                    started_at,
                },
                LiveProcess::Gone => LiveProcess::Gone,
            }
        }
        let probe = Probe {
            live: renamed,
            ..real_probe()
        };

        // Renamed, same start time: still the same process, still running.
        assert!(matches!(
            verdict(&probe, child.id(), Some(&identity)),
            Verdict::Renamed(_)
        ));
        assert!(matches!(
            wait_for_exit(
                &probe,
                child.id(),
                Some(&identity),
                Duration::from_millis(200)
            ),
            Wait::StillRunning
        ));
        // And it is not signalled under a name the user never confirmed.
        let result = stop_process(&probe, child.id(), true, Some(&identity));
        let still_running = child.try_wait().expect("try_wait").is_none();
        let _ = child.kill();
        let _ = child.wait();

        assert!(result
            .unwrap_err()
            .contains("now belongs to \"shutting-down\""));
        assert!(still_running);
    }

    // Nothing is signalled, and nothing escalated, on a check that could not
    // be made.
    #[test]
    fn an_identity_that_cannot_be_read_stops_nothing() {
        let mut child = sleeper();
        let identity = identity_of(child.id());
        let probe = Probe {
            live: |_| LiveProcess::Running {
                name: None,
                started_at: 0,
            },
            ..real_probe()
        };

        let result = stop_process(&probe, child.id(), true, Some(&identity));
        let waited = wait_for_exit(
            &probe,
            child.id(),
            Some(&identity),
            Duration::from_millis(100),
        );
        let still_running = child.try_wait().expect("try_wait").is_none();
        let _ = child.kill();
        let _ = child.wait();

        assert!(result.unwrap_err().contains("could not be confirmed"));
        assert!(matches!(waited, Wait::Unconfirmed(_)));
        assert!(still_running);
    }

    #[test]
    fn failing_to_signal_a_live_process_is_an_error() {
        // SAFETY: geteuid has no preconditions.
        if unsafe { libc::geteuid() } == 0 {
            return; // root may signal anything
        }
        // Signal 0 only probes permission; PID 1 is never really signalled.
        let error = send_signal(1, 0, "0").unwrap_err();
        assert!(error.contains("not permitted"), "{error}");
    }

    #[test]
    fn pids_that_mean_a_group_to_kill_are_rejected() {
        // kill(0) is this process group and kill(-1) is everything the user
        // owns; u32::MAX is -1 as a pid_t. Signal 0 would be harmless, but
        // none of these may reach kill at all.
        for pid in [0, u32::MAX, u32::MAX - 1, i32::MAX as u32 + 1] {
            let error = send_signal(pid, 0, "0").unwrap_err();
            assert!(error.contains("Invalid PID"), "{pid}: {error}");
        }
    }
}
