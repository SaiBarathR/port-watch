//! Decides whether a stop or a delete may go ahead, from what the latest scan
//! knows about the PID. The webview only names a PID (and for a delete, the
//! folder it showed); everything else is taken from the scan.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::app_settings::AppSettings;
use crate::platform;
use crate::platform::path_validation::{folder_identity, DeleteRules, FolderIdentity};
use crate::poller::PortPoller;
use crate::scanner::{PortProcess, ProcessIdentity};

fn not_listed(pid: u32) -> String {
    format!("PID {pid} is not in the latest scan. Refresh and try again.")
}

fn assert_system_actions_allowed(
    process: &PortProcess,
    allow_system_actions: bool,
) -> Result<(), String> {
    if process.is_system_service && !allow_system_actions {
        return Err(
            "System process actions are disabled. Enable them in Settings to continue.".into(),
        );
    }

    Ok(())
}

/// What was on the row the caller acted on.
#[derive(Debug, Clone, Copy, Default)]
pub struct SeenProcess<'a> {
    pub name: Option<&'a str>,
    /// Unix seconds; None or 0 when the caller does not know.
    pub started_at: Option<u64>,
}

// If the scan now has a different process under that PID, the row was stale.
// A different name shows it, and so does a different start time: the same
// program can be handed a PID its earlier run once had.
fn assert_same_process(process: &PortProcess, seen: SeenProcess) -> Result<(), String> {
    let stale = |now: String, was: &str| {
        Err(format!(
            "PID {} now belongs to {now}, not {was} — the process list was stale. Refresh and try again.",
            process.pid
        ))
    };

    if let Some(name) = seen.name {
        if name != process.name {
            return stale(format!("\"{}\"", process.name), &format!("\"{name}\""));
        }
    }
    if let Some(started_at) = seen.started_at.filter(|started_at| *started_at != 0) {
        if process.started_at != 0 && process.started_at != started_at {
            return stale(
                format!("a newer \"{}\"", process.name),
                "the one that was listed",
            );
        }
    }

    Ok(())
}

#[derive(Debug, PartialEq)]
pub enum StopTarget {
    /// In the latest scan; the live process must still be this one.
    Listed { identity: ProcessIdentity },
    /// Not in the latest scan and no longer running: nothing left to stop.
    AlreadyGone,
}

pub fn plan_stop(
    pid: u32,
    listed: Option<&PortProcess>,
    seen: SeenProcess,
    allow_system_actions: bool,
    is_running: impl FnOnce() -> bool,
) -> Result<StopTarget, String> {
    let Some(process) = listed else {
        // Nothing says what this PID is, so there is no telling whether it is
        // a system service or even the process the caller meant.
        return if is_running() {
            Err(not_listed(pid))
        } else {
            Ok(StopTarget::AlreadyGone)
        };
    };

    assert_same_process(process, seen)?;
    assert_system_actions_allowed(process, allow_system_actions)?;
    Ok(StopTarget::Listed {
        identity: process.identity(),
    })
}

pub fn stop_process(
    app: &AppHandle,
    pid: u32,
    force: bool,
    seen: SeenProcess,
) -> Result<(), String> {
    if pid == 0 {
        return Err("Invalid PID".into());
    }

    let listed = app.state::<PortPoller>().find_by_pid(pid);
    let allow_system_actions = app.state::<AppSettings>().allow_system_process_actions();
    let target = plan_stop(pid, listed.as_ref(), seen, allow_system_actions, || {
        platform::shell::is_running(pid)
    })?;

    match target {
        StopTarget::AlreadyGone => Ok(()),
        StopTarget::Listed { identity } => {
            platform::shell::stop_process(pid, force, Some(&identity))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeleteMode {
    Trash,
    Permanent,
}

pub struct DeleteRequest<'a> {
    pub expected_name: &'a str,
    pub expected_started_at: Option<u64>,
    /// The folder the user was shown.
    pub path: &'a str,
    pub mode: DeleteMode,
    /// The folder name typed to confirm a permanent delete.
    pub confirmation: Option<&'a str>,
}

/// A folder that passed every check, and what it was at that moment.
#[derive(Debug, PartialEq)]
struct DeleteTarget {
    folder: PathBuf,
    identity: FolderIdentity,
}

/// Everything that must hold before a process is stopped and its project
/// folder deleted. Returns the canonical folder.
fn plan_delete(
    pid: u32,
    listed: Option<&PortProcess>,
    request: &DeleteRequest,
    allow_system_actions: bool,
    rules: &DeleteRules,
) -> Result<DeleteTarget, String> {
    let process = listed.ok_or_else(|| not_listed(pid))?;
    assert_same_process(
        process,
        SeenProcess {
            name: Some(request.expected_name),
            started_at: request.expected_started_at,
        },
    )?;
    assert_system_actions_allowed(process, allow_system_actions)?;

    if let Some(reason) = &process.delete_blocked {
        return Err(reason.clone());
    }

    let requested = Path::new(request.path);
    let folder = match request.mode {
        DeleteMode::Trash => rules.resolve(requested)?,
        DeleteMode::Permanent => {
            rules.resolve_permanent(requested, request.confirmation.unwrap_or_default())?
        }
    };

    // The webview names the folder, but only the scan says which folder is
    // this process's.
    let own = std::fs::canonicalize(process.project_dir()).ok();
    if own.as_deref() != Some(folder.as_path()) {
        return Err(format!(
            "{} is not the project folder of PID {pid}. Refresh and try again.",
            folder.display()
        ));
    }

    let identity =
        folder_identity(&folder).map_err(|err| format!("Failed to inspect the folder: {err}"))?;
    Ok(DeleteTarget { folder, identity })
}

/// Stops the process, then removes the folder. Nothing is removed unless the
/// stop succeeded and the folder is still the one that was checked.
fn run_delete(
    rules: &DeleteRules,
    target: &DeleteTarget,
    stop: impl FnOnce() -> Result<(), String>,
    remove: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    stop().map_err(|err| format!("Nothing was deleted: {err}"))?;

    // The stop can take seconds. The path must still pass, and must still
    // lead to the same directory: another one moved into its place is not
    // what the user confirmed.
    let still_there = rules
        .resolve(&target.folder)
        .map_err(|err| format!("The process was stopped, but its folder was not deleted: {err}"))?;
    if still_there != target.folder
        || folder_identity(&still_there).ok().as_ref() != Some(&target.identity)
    {
        return Err(
            "The process was stopped, but its folder was replaced meanwhile and was not deleted."
                .into(),
        );
    }

    remove(&target.folder).map_err(|err| {
        format!("The process was stopped, but its folder could not be deleted: {err}")
    })
}

fn remove_folder(folder: &Path, mode: DeleteMode) -> Result<(), String> {
    match mode {
        DeleteMode::Trash => trash::delete(folder).map_err(|err| err.to_string()),
        DeleteMode::Permanent => std::fs::remove_dir_all(folder).map_err(|err| err.to_string()),
    }
}

/// Checks everything first, then stops the process, then removes its folder.
fn delete_with(
    pid: u32,
    listed: Option<&PortProcess>,
    request: &DeleteRequest,
    allow_system_actions: bool,
    rules: &DeleteRules,
    stop: impl FnOnce(&ProcessIdentity) -> Result<(), String>,
    remove: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let target = plan_delete(pid, listed, request, allow_system_actions, rules)?;
    let identity = listed.ok_or_else(|| not_listed(pid))?.identity();
    run_delete(rules, &target, || stop(&identity), remove)
}

pub fn delete_project(app: &AppHandle, pid: u32, request: &DeleteRequest) -> Result<(), String> {
    if pid == 0 {
        return Err("Invalid PID".into());
    }

    let listed = app.state::<PortPoller>().find_by_pid(pid);
    let allow_system_actions = app.state::<AppSettings>().allow_system_process_actions();
    let rules = DeleteRules::for_current_user()?;

    delete_with(
        pid,
        listed.as_ref(),
        request,
        allow_system_actions,
        &rules,
        |identity| platform::shell::stop_process(pid, false, Some(identity)),
        |folder| remove_folder(folder, request.mode),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classifier::SystemKind;
    use crate::scanner::PortBinding;
    use std::cell::Cell;
    use std::fs;

    fn not_protected(_: &Path) -> bool {
        false
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        home: PathBuf,
        project: PathBuf,
        rules: DeleteRules,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let home = fs::canonicalize(dir.path()).unwrap().join("home");
            let project = home.join("Dev/my-project");
            fs::create_dir_all(&project).unwrap();
            fs::write(project.join("package.json"), "{}").unwrap();
            let rules = DeleteRules::new(home.clone(), not_protected);
            Self {
                _dir: dir,
                home,
                project,
                rules,
            }
        }

        fn process(&self, pid: u32, name: &str) -> PortProcess {
            let project = self.project.to_string_lossy().into_owned();
            PortProcess {
                id: String::new(),
                pid,
                name: name.into(),
                user: "dev".into(),
                ports: vec![PortBinding {
                    address: "*".into(),
                    port: 3000,
                    protocol: "TCP".into(),
                }],
                executable_path: "/usr/local/bin/node".into(),
                script_path: None,
                command_line: "node server.js".into(),
                working_directory: project.clone(),
                project_root: project,
                system_kind: SystemKind::User,
                is_system_service: false,
                started_at: 1_790_000_000,
                delete_blocked: None,
            }
        }

        // The project folder as a delete that passed its checks would hold it.
        fn target(&self) -> DeleteTarget {
            DeleteTarget {
                folder: self.project.clone(),
                identity: folder_identity(&self.project).unwrap(),
            }
        }

        fn request<'a>(&self, path: &'a str, mode: DeleteMode) -> DeleteRequest<'a> {
            DeleteRequest {
                expected_name: "node",
                expected_started_at: None,
                path,
                mode,
                confirmation: None,
            }
        }
    }

    fn system(mut process: PortProcess) -> PortProcess {
        process.system_kind = SystemKind::System;
        process.is_system_service = true;
        process
    }

    #[cfg(unix)]
    fn live_started_at(pid: u32) -> u64 {
        use crate::platform::unix::LiveProcess;
        match (platform::shell::probe().live)(pid) {
            LiveProcess::Running { started_at, .. } => started_at,
            LiveProcess::Gone => panic!("PID {pid} should be running"),
        }
    }

    fn must_not_probe() -> bool {
        panic!("a listed process must not be probed");
    }

    fn seen(name: &str) -> SeenProcess<'_> {
        SeenProcess {
            name: Some(name),
            started_at: None,
        }
    }

    // --- stop ---------------------------------------------------------------

    #[test]
    fn stop_targets_a_listed_user_process_by_its_scanned_name() {
        let fixture = Fixture::new();
        let process = fixture.process(42, "node");
        for seen in [seen("node"), SeenProcess::default()] {
            assert_eq!(
                plan_stop(42, Some(&process), seen, false, must_not_probe),
                Ok(StopTarget::Listed {
                    identity: ProcessIdentity {
                        name: "node".into(),
                        started_at: 1_790_000_000,
                    }
                })
            );
        }
    }

    #[test]
    fn stop_treats_an_unlisted_dead_pid_as_already_gone() {
        assert_eq!(
            plan_stop(42, None, seen("node"), false, || false),
            Ok(StopTarget::AlreadyGone)
        );
    }

    #[test]
    fn stop_refuses_an_unlisted_pid_that_is_still_running() {
        // Even with system actions allowed: nothing identifies this process.
        for allow in [false, true] {
            let err = plan_stop(42, None, seen("node"), allow, || true).unwrap_err();
            assert!(err.contains("not in the latest scan"), "{err}");
        }
    }

    #[test]
    fn stop_refuses_a_pid_the_scan_knows_under_another_name() {
        let fixture = Fixture::new();
        let process = fixture.process(42, "postgres");
        let err = plan_stop(42, Some(&process), seen("node"), true, must_not_probe).unwrap_err();
        assert!(err.contains("now belongs to \"postgres\""), "{err}");
    }

    #[test]
    fn stop_needs_the_opt_in_for_system_services() {
        let fixture = Fixture::new();
        let process = system(fixture.process(42, "node"));
        let err = plan_stop(42, Some(&process), seen("node"), false, must_not_probe).unwrap_err();
        assert!(err.contains("System process actions are disabled"), "{err}");
        assert!(plan_stop(42, Some(&process), seen("node"), true, must_not_probe).is_ok());
    }

    // The same program under the same PID, started at another time: the row
    // the caller saw was a different process.
    #[test]
    fn stop_refuses_a_row_for_an_earlier_process_with_that_pid_and_name() {
        let fixture = Fixture::new();
        let process = fixture.process(42, "node");
        let earlier = SeenProcess {
            name: Some("node"),
            started_at: Some(process.started_at - 60),
        };
        let err = plan_stop(42, Some(&process), earlier, true, must_not_probe).unwrap_err();
        assert!(err.contains("a newer \"node\""), "{err}");

        // The same start time, or none on either side, is not a mismatch.
        for started_at in [Some(process.started_at), Some(0), None] {
            let same = SeenProcess {
                name: Some("node"),
                started_at,
            };
            assert!(plan_stop(42, Some(&process), same, true, must_not_probe).is_ok());
        }
        let mut unknown = fixture.process(42, "node");
        unknown.started_at = 0;
        assert!(plan_stop(42, Some(&unknown), earlier, true, must_not_probe).is_ok());
    }

    // --- delete -------------------------------------------------------------

    #[test]
    fn delete_resolves_the_process_own_project_folder() {
        let fixture = Fixture::new();
        let process = fixture.process(42, "node");
        let path = fixture.project.to_string_lossy();
        assert_eq!(
            plan_delete(
                42,
                Some(&process),
                &fixture.request(&path, DeleteMode::Trash),
                false,
                &fixture.rules
            ),
            Ok(fixture.target())
        );
    }

    #[test]
    fn delete_falls_back_to_the_working_directory_without_a_project_root() {
        let fixture = Fixture::new();
        let mut process = fixture.process(42, "node");
        process.project_root = String::new();
        let path = fixture.project.to_string_lossy();
        assert!(plan_delete(
            42,
            Some(&process),
            &fixture.request(&path, DeleteMode::Trash),
            false,
            &fixture.rules
        )
        .is_ok());
    }

    #[test]
    fn delete_refuses_a_pid_that_left_the_scan() {
        let fixture = Fixture::new();
        let path = fixture.project.to_string_lossy();
        let err = plan_delete(
            42,
            None,
            &fixture.request(&path, DeleteMode::Trash),
            true,
            &fixture.rules,
        )
        .unwrap_err();
        assert!(err.contains("not in the latest scan"), "{err}");
    }

    #[test]
    fn delete_refuses_a_stale_row() {
        let fixture = Fixture::new();
        let process = fixture.process(42, "postgres");
        let path = fixture.project.to_string_lossy();
        let err = plan_delete(
            42,
            Some(&process),
            &fixture.request(&path, DeleteMode::Trash),
            true,
            &fixture.rules,
        )
        .unwrap_err();
        assert!(err.contains("now belongs to \"postgres\""), "{err}");
    }

    #[test]
    fn delete_needs_the_opt_in_for_system_services() {
        let fixture = Fixture::new();
        let process = system(fixture.process(42, "node"));
        let path = fixture.project.to_string_lossy();
        let request = fixture.request(&path, DeleteMode::Trash);
        let err = plan_delete(42, Some(&process), &request, false, &fixture.rules).unwrap_err();
        assert!(err.contains("System process actions are disabled"), "{err}");
        assert!(plan_delete(42, Some(&process), &request, true, &fixture.rules).is_ok());
    }

    #[test]
    fn delete_refuses_a_folder_that_is_not_the_process_own() {
        let fixture = Fixture::new();
        let process = fixture.process(42, "node");
        let other = fixture.home.join("Dev/other-project");
        fs::create_dir_all(&other).unwrap();

        for path in [&other, &fixture.home.join("Dev")] {
            let path = path.to_string_lossy();
            let err = plan_delete(
                42,
                Some(&process),
                &fixture.request(&path, DeleteMode::Trash),
                false,
                &fixture.rules,
            )
            .unwrap_err();
            assert!(err.contains("is not the project folder of PID 42"), "{err}");
        }
    }

    #[test]
    fn delete_refuses_what_the_scan_marked_as_blocked() {
        let fixture = Fixture::new();
        let mut process = fixture.process(42, "node");
        process.delete_blocked = Some("Guessed from the program's location.".into());
        let path = fixture.project.to_string_lossy();
        let err = plan_delete(
            42,
            Some(&process),
            &fixture.request(&path, DeleteMode::Trash),
            false,
            &fixture.rules,
        )
        .unwrap_err();
        assert_eq!(err, "Guessed from the program's location.");
    }

    #[test]
    fn delete_refuses_a_working_directory_outside_home() {
        // A server started from `/`: the folder is its own, and still not
        // something to delete.
        let fixture = Fixture::new();
        let outside = fixture.home.parent().unwrap().join("srv");
        fs::create_dir_all(&outside).unwrap();
        let mut process = fixture.process(42, "node");
        process.project_root = String::new();
        process.working_directory = outside.to_string_lossy().into_owned();

        let path = outside.to_string_lossy();
        let err = plan_delete(
            42,
            Some(&process),
            &fixture.request(&path, DeleteMode::Trash),
            false,
            &fixture.rules,
        )
        .unwrap_err();
        assert!(err.contains("outside your home folder"), "{err}");
    }

    #[test]
    fn permanent_delete_needs_the_typed_folder_name() {
        let fixture = Fixture::new();
        let process = fixture.process(42, "node");
        let path = fixture.project.to_string_lossy();

        for confirmation in [None, Some(""), Some("my-projec")] {
            let request = DeleteRequest {
                confirmation,
                ..fixture.request(&path, DeleteMode::Permanent)
            };
            let err = plan_delete(42, Some(&process), &request, false, &fixture.rules).unwrap_err();
            assert!(err.contains("Confirmation must match"), "{err}");
        }

        let request = DeleteRequest {
            confirmation: Some("my-project"),
            ..fixture.request(&path, DeleteMode::Permanent)
        };
        assert!(plan_delete(42, Some(&process), &request, false, &fixture.rules).is_ok());
    }

    #[test]
    fn nothing_is_removed_when_the_stop_fails() {
        let fixture = Fixture::new();
        let removed = Cell::new(false);
        let err = run_delete(
            &fixture.rules,
            &fixture.target(),
            || Err("PID 42 is still running after SIGKILL".into()),
            |_| {
                removed.set(true);
                Ok(())
            },
        )
        .unwrap_err();

        assert!(err.starts_with("Nothing was deleted"), "{err}");
        assert!(!removed.get());
        assert!(fixture.project.exists());
    }

    #[test]
    fn nothing_is_removed_when_the_folder_changed_during_the_stop() {
        let fixture = Fixture::new();
        let removed = Cell::new(false);
        let err = run_delete(
            &fixture.rules,
            &fixture.target(),
            || {
                fs::remove_dir_all(&fixture.project).unwrap();
                Ok(())
            },
            |_| {
                removed.set(true);
                Ok(())
            },
        )
        .unwrap_err();

        assert!(err.contains("was not deleted"), "{err}");
        assert!(!removed.get());
    }

    // The path is the same and still passes every rule, but it is a different
    // directory from the one the user confirmed.
    #[test]
    fn nothing_is_removed_when_another_folder_took_its_place_during_the_stop() {
        let fixture = Fixture::new();
        let moved_away = fixture.home.join("Dev/moved-away");
        let removed = Cell::new(false);
        let err = run_delete(
            &fixture.rules,
            &fixture.target(),
            || {
                // Long enough for a new creation time where that is the identity.
                std::thread::sleep(std::time::Duration::from_millis(20));
                fs::rename(&fixture.project, &moved_away).unwrap();
                fs::create_dir(&fixture.project).unwrap();
                fs::write(fixture.project.join("notes.txt"), "keep").unwrap();
                Ok(())
            },
            |_| {
                removed.set(true);
                Ok(())
            },
        )
        .unwrap_err();

        assert!(err.contains("was replaced meanwhile"), "{err}");
        assert!(!removed.get());
        assert!(fixture.project.join("notes.txt").exists());
        assert!(moved_away.join("package.json").exists());
    }

    #[test]
    fn delete_refuses_a_row_for_an_earlier_process_with_that_pid_and_name() {
        let fixture = Fixture::new();
        let process = fixture.process(42, "node");
        let path = fixture.project.to_string_lossy();
        let request = DeleteRequest {
            expected_started_at: Some(process.started_at - 60),
            ..fixture.request(&path, DeleteMode::Trash)
        };
        let err = plan_delete(42, Some(&process), &request, false, &fixture.rules).unwrap_err();
        assert!(err.contains("a newer \"node\""), "{err}");
    }

    #[test]
    fn a_failed_removal_says_the_process_was_stopped() {
        let fixture = Fixture::new();
        let err = run_delete(
            &fixture.rules,
            &fixture.target(),
            || Ok(()),
            |_| Err("permission denied".into()),
        )
        .unwrap_err();
        assert!(err.contains("was stopped, but its folder could not be deleted"));
    }

    // The whole flow against a real process, with the real stop.
    #[test]
    #[cfg(unix)]
    fn deletes_a_running_process_and_its_folder() {
        let fixture = Fixture::new();
        let mut child = crate::platform::unix::testing::spawn_sleep(Some(&fixture.project));
        let pid = child.id();
        let mut process = fixture.process(pid, "sleep");
        process.started_at = live_started_at(pid);
        let path = fixture.project.to_string_lossy();
        let request = DeleteRequest {
            expected_name: "sleep",
            confirmation: Some("my-project"),
            ..fixture.request(&path, DeleteMode::Permanent)
        };

        let result = delete_with(
            pid,
            Some(&process),
            &request,
            false,
            &fixture.rules,
            |identity| platform::shell::stop_process(pid, false, Some(identity)),
            |folder| remove_folder(folder, DeleteMode::Permanent),
        );
        let exited = child.wait().is_ok();

        assert_eq!(result, Ok(()));
        assert!(exited);
        assert!(!fixture.project.exists());
    }

    // The bug this module replaced: the process was stopped first and the
    // path checked second.
    #[test]
    #[cfg(unix)]
    fn a_refused_delete_leaves_the_process_running() {
        let fixture = Fixture::new();
        let outside = fixture.home.parent().unwrap().join("srv");
        fs::create_dir_all(&outside).unwrap();
        let mut child = crate::platform::unix::testing::spawn_sleep(Some(&outside));
        let pid = child.id();
        let mut process = fixture.process(pid, "sleep");
        process.started_at = live_started_at(pid);
        process.project_root = String::new();
        process.working_directory = outside.to_string_lossy().into_owned();

        let path = outside.to_string_lossy();
        let request = DeleteRequest {
            expected_name: "sleep",
            ..fixture.request(&path, DeleteMode::Trash)
        };
        let result = delete_with(
            pid,
            Some(&process),
            &request,
            false,
            &fixture.rules,
            |identity| platform::shell::stop_process(pid, false, Some(identity)),
            |folder| remove_folder(folder, DeleteMode::Permanent),
        );
        let still_running = child.try_wait().expect("try_wait").is_none();
        let _ = child.kill();
        let _ = child.wait();

        assert!(result.unwrap_err().contains("outside your home folder"));
        assert!(still_running);
        assert!(outside.exists());
    }
}
