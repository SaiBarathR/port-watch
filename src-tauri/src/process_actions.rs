//! Decides whether a stop or a delete may go ahead, from what the latest scan
//! knows about the PID. The webview only names a PID (and for a delete, the
//! folder it showed); everything else is taken from the scan.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::app_settings::AppSettings;
use crate::platform;
use crate::platform::path_validation::DeleteRules;
use crate::poller::PortPoller;
use crate::scanner::PortProcess;

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

// The caller acted on a row it was shown. If the scan now has a different
// program under that PID, the row was stale.
fn assert_same_process(process: &PortProcess, expected_name: Option<&str>) -> Result<(), String> {
    match expected_name {
        Some(expected) if expected != process.name => Err(format!(
            "PID {} now belongs to \"{}\", not \"{expected}\" — the process list was stale. Refresh and try again.",
            process.pid, process.name
        )),
        _ => Ok(()),
    }
}

#[derive(Debug, PartialEq)]
pub enum StopTarget {
    /// In the latest scan under this name; the live process must still have it.
    Listed { name: String },
    /// Not in the latest scan and no longer running: nothing left to stop.
    AlreadyGone,
}

pub fn plan_stop(
    pid: u32,
    listed: Option<&PortProcess>,
    expected_name: Option<&str>,
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

    assert_same_process(process, expected_name)?;
    assert_system_actions_allowed(process, allow_system_actions)?;
    Ok(StopTarget::Listed {
        name: process.name.clone(),
    })
}

pub fn stop_process(
    app: &AppHandle,
    pid: u32,
    force: bool,
    expected_name: Option<&str>,
) -> Result<(), String> {
    if pid == 0 {
        return Err("Invalid PID".into());
    }

    let listed = app.state::<PortPoller>().find_by_pid(pid);
    let allow_system_actions = app.state::<AppSettings>().allow_system_process_actions();
    let target = plan_stop(
        pid,
        listed.as_ref(),
        expected_name,
        allow_system_actions,
        || platform::shell::current_process_name(pid).is_some(),
    )?;

    match target {
        StopTarget::AlreadyGone => Ok(()),
        StopTarget::Listed { name } => platform::shell::stop_process(pid, force, Some(&name)),
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
    /// The folder the user was shown.
    pub path: &'a str,
    pub mode: DeleteMode,
    /// The folder name typed to confirm a permanent delete.
    pub confirmation: Option<&'a str>,
}

/// Everything that must hold before a process is stopped and its project
/// folder deleted. Returns the canonical folder.
fn plan_delete(
    pid: u32,
    listed: Option<&PortProcess>,
    request: &DeleteRequest,
    allow_system_actions: bool,
    rules: &DeleteRules,
) -> Result<PathBuf, String> {
    let process = listed.ok_or_else(|| not_listed(pid))?;
    assert_same_process(process, Some(request.expected_name))?;
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

    Ok(folder)
}

/// Stops the process, then removes `folder`. Nothing is removed unless the
/// stop succeeded.
fn run_delete(
    rules: &DeleteRules,
    folder: &Path,
    stop: impl FnOnce() -> Result<(), String>,
    remove: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    stop().map_err(|err| format!("Nothing was deleted: {err}"))?;

    // The stop can take seconds. Make sure the folder is still the one that
    // was checked before it.
    let still_there = rules
        .resolve(folder)
        .map_err(|err| format!("The process was stopped, but its folder was not deleted: {err}"))?;
    if still_there != folder {
        return Err("The process was stopped, but its folder changed and was not deleted.".into());
    }

    remove(folder).map_err(|err| {
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
    stop: impl FnOnce() -> Result<(), String>,
    remove: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let folder = plan_delete(pid, listed, request, allow_system_actions, rules)?;
    run_delete(rules, &folder, stop, remove)
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
        || platform::shell::stop_process(pid, false, Some(request.expected_name)),
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
                uptime_seconds: 60,
                delete_blocked: None,
            }
        }

        fn request<'a>(&self, path: &'a str, mode: DeleteMode) -> DeleteRequest<'a> {
            DeleteRequest {
                expected_name: "node",
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

    fn must_not_probe() -> bool {
        panic!("a listed process must not be probed");
    }

    // --- stop ---------------------------------------------------------------

    #[test]
    fn stop_targets_a_listed_user_process_by_its_scanned_name() {
        let fixture = Fixture::new();
        let process = fixture.process(42, "node");
        for expected in [Some("node"), None] {
            assert_eq!(
                plan_stop(42, Some(&process), expected, false, must_not_probe),
                Ok(StopTarget::Listed {
                    name: "node".into()
                })
            );
        }
    }

    #[test]
    fn stop_treats_an_unlisted_dead_pid_as_already_gone() {
        assert_eq!(
            plan_stop(42, None, Some("node"), false, || false),
            Ok(StopTarget::AlreadyGone)
        );
    }

    #[test]
    fn stop_refuses_an_unlisted_pid_that_is_still_running() {
        // Even with system actions allowed: nothing identifies this process.
        for allow in [false, true] {
            let err = plan_stop(42, None, Some("node"), allow, || true).unwrap_err();
            assert!(err.contains("not in the latest scan"), "{err}");
        }
    }

    #[test]
    fn stop_refuses_a_pid_the_scan_knows_under_another_name() {
        let fixture = Fixture::new();
        let process = fixture.process(42, "postgres");
        let err = plan_stop(42, Some(&process), Some("node"), true, must_not_probe).unwrap_err();
        assert!(err.contains("now belongs to \"postgres\""), "{err}");
    }

    #[test]
    fn stop_needs_the_opt_in_for_system_services() {
        let fixture = Fixture::new();
        let process = system(fixture.process(42, "node"));
        let err = plan_stop(42, Some(&process), Some("node"), false, must_not_probe).unwrap_err();
        assert!(err.contains("System process actions are disabled"), "{err}");
        assert!(plan_stop(42, Some(&process), Some("node"), true, must_not_probe).is_ok());
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
            Ok(fixture.project.clone())
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
            &fixture.project,
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
            &fixture.project,
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

    #[test]
    fn a_failed_removal_says_the_process_was_stopped() {
        let fixture = Fixture::new();
        let err = run_delete(
            &fixture.rules,
            &fixture.project,
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
        let mut child = std::process::Command::new("sleep")
            .arg("60")
            .current_dir(&fixture.project)
            .spawn()
            .expect("spawn sleep");
        let pid = child.id();
        let process = fixture.process(pid, "sleep");
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
            || platform::shell::stop_process(pid, false, Some("sleep")),
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
        let mut child = std::process::Command::new("sleep")
            .arg("60")
            .current_dir(&outside)
            .spawn()
            .expect("spawn sleep");
        let pid = child.id();
        let mut process = fixture.process(pid, "sleep");
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
            || platform::shell::stop_process(pid, false, Some("sleep")),
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
