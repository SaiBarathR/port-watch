//! Deciding whether the process under a PID is still the one a scan listed.
//!
//! A PID is recycled, so before anything is signalled the live process is
//! compared with what the scan recorded: its name, and its start time, which
//! is what tells a process from a later run of the same program. When that
//! comparison cannot be made, the answer is "unconfirmed", never "the same".

use crate::scanner::ProcessIdentity;

/// What a PID is right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveProcess {
    /// No such process, or one that has exited and only waits to be reaped.
    Gone,
    Running {
        /// None when this user may not inspect the process.
        name: Option<String>,
        /// Unix seconds; 0 when it could not be read.
        started_at: u64,
    },
}

/// How a platform reads a PID and compares what it finds with a scan.
pub struct Probe {
    pub live: fn(u32) -> LiveProcess,
    pub names_match: fn(&str, &str) -> bool,
    /// How many seconds two readings of one process's start time may differ
    /// by. Zero where the scan and the probe read it from the same place.
    pub start_slack: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Gone,
    /// The process the scan listed.
    Same,
    /// The listed process, under another name: it renamed itself or replaced
    /// its program. Still running, and no longer what the user was shown.
    Renamed(String),
    /// Something else holds the PID. `certain` when the start times prove
    /// it; otherwise only the name differs and no start time was available
    /// to say whether this is another process or the same one renamed.
    Other {
        now: String,
        certain: bool,
    },
    /// Something runs under the PID and nothing confirms it is the listed
    /// process. Says what was missing.
    Unconfirmed(String),
}

impl Verdict {
    /// Why a stop must not go ahead, for anything but `Same` and `Gone`.
    pub fn refusal(&self, pid: u32, expected: &ProcessIdentity) -> Option<String> {
        let was = &expected.name;
        match self {
            Verdict::Same | Verdict::Gone => None,
            Verdict::Renamed(now) | Verdict::Other { now, .. } => Some(format!(
                "PID {pid} now belongs to {now}, not \"{was}\" — the process list was stale. Refresh and try again."
            )),
            Verdict::Unconfirmed(why) => Some(format!(
                "PID {pid} could not be confirmed to still be \"{was}\" ({why}), so it was left alone. Refresh and try again."
            )),
        }
    }
}

pub fn verdict(probe: &Probe, pid: u32, expected: Option<&ProcessIdentity>) -> Verdict {
    let LiveProcess::Running { name, started_at } = (probe.live)(pid) else {
        return Verdict::Gone;
    };
    let Some(expected) = expected else {
        return Verdict::Same;
    };

    // Some(true): the very process the scan saw. Some(false): another one.
    // None: the scan had no start time, so only the name can speak.
    let same_start = if expected.started_at == 0 {
        None
    } else if started_at == 0 {
        return Verdict::Unconfirmed("its start time could not be read".into());
    } else {
        Some(started_at.abs_diff(expected.started_at) <= probe.start_slack)
    };

    match (name, same_start) {
        (name, Some(false)) => Verdict::Other {
            now: match name {
                Some(name) => format!("a newer \"{name}\""),
                None => "a newer process".into(),
            },
            certain: true,
        },
        (Some(name), _) if (probe.names_match)(&name, &expected.name) => Verdict::Same,
        (Some(name), Some(true)) => Verdict::Renamed(format!("\"{name}\"")),
        (Some(name), _) => Verdict::Other {
            now: format!("\"{name}\""),
            certain: false,
        },
        // Its name is not ours to read, but the start time vouches for it.
        (None, Some(true)) => Verdict::Same,
        (None, _) => Verdict::Unconfirmed("it cannot be inspected".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected(name: &str, started_at: u64) -> ProcessIdentity {
        ProcessIdentity {
            name: name.into(),
            started_at,
        }
    }

    fn check(live: fn(u32) -> LiveProcess, expected: &ProcessIdentity, slack: u64) -> Verdict {
        let probe = Probe {
            live,
            names_match: |live, scanned| live == scanned,
            start_slack: slack,
        };
        verdict(&probe, 42, Some(expected))
    }

    fn running(name: Option<&str>, started_at: u64) -> LiveProcess {
        LiveProcess::Running {
            name: name.map(str::to_string),
            started_at,
        }
    }

    #[test]
    fn the_listed_process_is_the_same() {
        assert_eq!(
            check(
                |_| running(Some("node"), 1_000),
                &expected("node", 1_000),
                0
            ),
            Verdict::Same
        );
    }

    #[test]
    fn a_missing_process_is_gone() {
        assert_eq!(
            check(|_| LiveProcess::Gone, &expected("node", 1_000), 0),
            Verdict::Gone
        );
    }

    #[test]
    fn a_later_start_time_is_another_process_whatever_its_name() {
        for live in [
            (|_| running(Some("node"), 1_060)) as fn(u32) -> LiveProcess,
            |_| running(Some("postgres"), 1_060),
            |_| running(None, 1_060),
        ] {
            assert!(matches!(
                check(live, &expected("node", 1_000), 0),
                Verdict::Other { certain: true, .. }
            ));
        }
    }

    #[test]
    fn the_same_start_time_under_another_name_is_a_rename_not_an_exit() {
        assert_eq!(
            check(
                |_| running(Some("shutting-down"), 1_000),
                &expected("node", 1_000),
                0
            ),
            Verdict::Renamed("\"shutting-down\"".into())
        );
    }

    #[test]
    fn an_unreadable_name_is_vouched_for_by_the_start_time() {
        assert_eq!(
            check(|_| running(None, 1_000), &expected("launchd", 1_000), 0),
            Verdict::Same
        );
    }

    // Nothing may be signalled on the strength of a check that did not happen.
    #[test]
    fn what_cannot_be_compared_is_unconfirmed_not_the_same() {
        // The scan knows when it started; the live process will not say.
        assert!(matches!(
            check(|_| running(Some("node"), 0), &expected("node", 1_000), 0),
            Verdict::Unconfirmed(_)
        ));
        assert!(matches!(
            check(|_| running(None, 0), &expected("node", 1_000), 0),
            Verdict::Unconfirmed(_)
        ));
        // The scan has no start time and the live name cannot be read.
        assert!(matches!(
            check(|_| running(None, 1_000), &expected("node", 0), 0),
            Verdict::Unconfirmed(_)
        ));
    }

    #[test]
    fn without_a_scanned_start_time_the_name_decides() {
        assert_eq!(
            check(|_| running(Some("node"), 1_060), &expected("node", 0), 0),
            Verdict::Same
        );
        assert_eq!(
            check(
                |_| running(Some("postgres"), 1_060),
                &expected("node", 0),
                0
            ),
            Verdict::Other {
                now: "\"postgres\"".into(),
                certain: false
            }
        );
    }

    #[test]
    fn slack_allows_two_sources_to_disagree_by_a_second() {
        let live = |_| running(Some("node.exe"), 1_001);
        assert_eq!(check(live, &expected("node.exe", 1_000), 1), Verdict::Same);
        assert!(matches!(
            check(live, &expected("node.exe", 1_000), 0),
            Verdict::Other { certain: true, .. }
        ));
        assert!(matches!(
            check(live, &expected("node.exe", 998), 1),
            Verdict::Other { certain: true, .. }
        ));
    }

    #[test]
    fn no_expectation_means_any_running_process_matches() {
        let probe = Probe {
            live: |_| running(None, 0),
            names_match: |live, scanned| live == scanned,
            start_slack: 0,
        };
        assert_eq!(verdict(&probe, 42, None), Verdict::Same);
    }

    #[test]
    fn refusals_say_what_was_found() {
        let node = expected("node", 1_000);
        assert_eq!(Verdict::Same.refusal(42, &node), None);
        assert_eq!(Verdict::Gone.refusal(42, &node), None);
        let stale = Verdict::Other {
            now: "\"postgres\"".into(),
            certain: false,
        }
        .refusal(42, &node)
        .unwrap();
        assert!(stale.contains("now belongs to \"postgres\", not \"node\""));
        let unconfirmed = Verdict::Unconfirmed("it cannot be inspected".into())
            .refusal(42, &node)
            .unwrap();
        assert!(unconfirmed.contains("could not be confirmed"));
    }
}
