#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::path::Path;
use std::path::PathBuf;
use std::process;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliInstallStatus {
    pub installed: bool,
    pub link_path: String,
    pub target_path: Option<String>,
    pub points_to_app: bool,
}

pub fn run_install_cli() {
    match install_cli_to_path() {
        Ok(()) => {
            match cli_link_path() {
                Ok(path) => println!("Installed {path}"),
                Err(err) => eprintln!("{err}"),
            }
            process::exit(0);
        }
        Err(err) => {
            eprintln!("{err}");
            process::exit(1);
        }
    }
}

fn current_app_executable() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|err| format!("Failed to resolve app executable: {err}"))
}

// ---------------------------------------------------------------------------
// macOS and Linux: a symlink to the app's executable
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
pub const CLI_LINK_PATH: &str = "/usr/local/bin/port-watch";

#[cfg(target_os = "macos")]
fn link_path() -> Result<PathBuf, String> {
    Ok(PathBuf::from(CLI_LINK_PATH))
}

#[cfg(target_os = "linux")]
fn link_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "Could not determine home directory".to_string())?;
    Ok(home.join(".local/bin/port-watch"))
}

// /usr/local/bin belongs to root on a stock Mac, so the link there is made and
// removed through an administrator prompt. ~/.local/bin never needs one.
#[cfg(target_os = "macos")]
const ESCALATE: Option<unix::Escalate> = Some(&run_as_admin);
#[cfg(target_os = "linux")]
const ESCALATE: Option<unix::Escalate> = None;

#[cfg(unix)]
pub fn cli_link_path() -> Result<String, String> {
    Ok(link_path()?.to_string_lossy().into_owned())
}

#[cfg(unix)]
pub fn get_cli_install_status() -> Result<CliInstallStatus, String> {
    Ok(unix::status(&link_path()?, &current_app_executable()?))
}

#[cfg(unix)]
pub fn install_cli_to_path() -> Result<(), String> {
    unix::install(&link_path()?, &current_app_executable()?, ESCALATE)
}

#[cfg(unix)]
pub fn uninstall_cli_from_path() -> Result<(), String> {
    unix::uninstall(&link_path()?, &current_app_executable()?, ESCALATE)
}

#[cfg(unix)]
mod unix {
    use super::CliInstallStatus;
    use std::io::ErrorKind;
    use std::path::{Path, PathBuf};

    /// A step that failed for lack of permission and can be retried with more.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum PrivilegedStep<'a> {
        Link { source: &'a Path, link: &'a Path },
        Unlink { link: &'a Path },
    }

    pub type Escalate<'a> = &'a (dyn Fn(PrivilegedStep) -> Result<(), String> + Sync);

    // What sits at the link path, if anything. `Path::exists()` follows
    // symlinks, so it would call a dangling link (the app moved or was
    // updated) "not installed" while that link still blocks a reinstall.
    enum Entry {
        Missing,
        NotASymlink,
        Link { target: String },
    }

    fn entry_at(link: &Path) -> Entry {
        match std::fs::symlink_metadata(link) {
            Err(_) => Entry::Missing,
            Ok(metadata) if !metadata.file_type().is_symlink() => Entry::NotASymlink,
            Ok(_) => match std::fs::read_link(link) {
                Ok(target) => Entry::Link {
                    target: target.to_string_lossy().into_owned(),
                },
                Err(_) => Entry::NotASymlink,
            },
        }
    }

    fn canonicalize_if_exists(path: &Path) -> Option<PathBuf> {
        if path.exists() {
            std::fs::canonicalize(path).ok()
        } else {
            Some(path.to_path_buf())
        }
    }

    fn same_file(left: &str, right: &Path) -> bool {
        match (
            canonicalize_if_exists(Path::new(left)),
            canonicalize_if_exists(right),
        ) {
            (Some(left_path), Some(right_path)) => left_path == right_path,
            _ => Path::new(left) == right,
        }
    }

    pub fn status(link: &Path, app_exe: &Path) -> CliInstallStatus {
        let link_path = link.to_string_lossy().into_owned();
        match entry_at(link) {
            Entry::Missing => CliInstallStatus {
                installed: false,
                link_path,
                target_path: None,
                points_to_app: false,
            },
            Entry::NotASymlink => CliInstallStatus {
                installed: true,
                link_path,
                target_path: None,
                points_to_app: false,
            },
            Entry::Link { target } => CliInstallStatus {
                installed: true,
                link_path,
                points_to_app: same_file(&target, app_exe),
                target_path: Some(target),
            },
        }
    }

    pub fn install(link: &Path, app_exe: &Path, escalate: Option<Escalate>) -> Result<(), String> {
        let replaces_dangling_link = match entry_at(link) {
            Entry::Missing => false,
            Entry::NotASymlink => {
                return Err(format!(
                    "{} exists but is not a symlink. Remove it manually and try again.",
                    link.display()
                ));
            }
            Entry::Link { target } if same_file(&target, app_exe) => return Ok(()),
            Entry::Link { target } if Path::new(&target).exists() => {
                return Err(format!(
                    "Another port-watch is installed at {} (points to {target})",
                    link.display()
                ));
            }
            // Left behind by a moved or updated app.
            Entry::Link { .. } => true,
        };

        let attempt = || -> Result<(), (&'static str, std::io::Error)> {
            if replaces_dangling_link {
                std::fs::remove_file(link).map_err(|err| ("replace the stale CLI link", err))?;
            }
            if let Some(parent) = link.parent() {
                std::fs::create_dir_all(parent).map_err(|err| ("create the link's folder", err))?;
            }
            std::os::unix::fs::symlink(app_exe, link).map_err(|err| ("create the CLI link", err))
        };

        // Any of the steps can be the one that lacks permission: the folder
        // may not exist yet, or may not be writable.
        match (attempt(), escalate) {
            (Ok(()), _) => Ok(()),
            (Err((_, err)), Some(escalate)) if err.kind() == ErrorKind::PermissionDenied => {
                escalate(PrivilegedStep::Link {
                    source: app_exe,
                    link,
                })
            }
            (Err((step, err)), _) => Err(format!("Failed to {step}: {err}")),
        }
    }

    pub fn uninstall(
        link: &Path,
        app_exe: &Path,
        escalate: Option<Escalate>,
    ) -> Result<(), String> {
        match entry_at(link) {
            Entry::Missing => return Ok(()),
            Entry::NotASymlink => {
                return Err(format!(
                    "{} exists but is not a symlink. Remove it manually.",
                    link.display()
                ));
            }
            // A dangling link is removable regardless of where it pointed.
            Entry::Link { target }
                if !same_file(&target, app_exe) && Path::new(&target).exists() =>
            {
                return Err(format!(
                    "{} points to {target}, not this app. Uninstall skipped.",
                    link.display()
                ));
            }
            Entry::Link { .. } => {}
        }

        match (std::fs::remove_file(link), escalate) {
            (Ok(()), _) => Ok(()),
            (Err(err), Some(escalate)) if err.kind() == ErrorKind::PermissionDenied => {
                escalate(PrivilegedStep::Unlink { link })
            }
            (Err(err), _) => Err(format!("Failed to remove CLI link: {err}")),
        }
    }
}

#[cfg(target_os = "macos")]
fn admin_shell_script(step: unix::PrivilegedStep) -> String {
    match step {
        unix::PrivilegedStep::Link { source, link } => {
            let folder = link.parent().unwrap_or(Path::new("/"));
            format!(
                "mkdir -p {} && ln -sf {} {}",
                shell_escape(&folder.to_string_lossy()),
                shell_escape(&source.to_string_lossy()),
                shell_escape(&link.to_string_lossy())
            )
        }
        unix::PrivilegedStep::Unlink { link } => {
            format!("rm -f {}", shell_escape(&link.to_string_lossy()))
        }
    }
}

#[cfg(target_os = "macos")]
fn run_as_admin(step: unix::PrivilegedStep) -> Result<(), String> {
    let osa_script = format!(
        "do shell script {} with administrator privileges",
        applescript_string(&admin_shell_script(step))
    );

    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(osa_script)
        .output()
        .map_err(|err| format!("Failed to run osascript: {err}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if message.is_empty() {
            Err("The administrator prompt was cancelled or failed.".to_string())
        } else {
            Err(message)
        }
    }
}

#[cfg(target_os = "macos")]
fn shell_escape(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(target_os = "macos")]
fn applescript_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

// ---------------------------------------------------------------------------
// Windows: a .cmd shim on the user's PATH that runs the installed app
// ---------------------------------------------------------------------------
//
// The app is a GUI-subsystem program: started straight from a prompt, the
// shell does not wait for it, so its output lands after the prompt returns
// and its exit code is lost. Inside a command script cmd.exe does wait, so
// the shim gives `port-watch check` its output in order and its exit code.
// Unlike the copy of the executable this replaced, it also cannot go stale
// when the app is updated.

#[cfg(any(target_os = "windows", test))]
const SHIM_NAME: &str = "port-watch.cmd";

// What earlier versions installed: a copy of the app's executable. It has to
// go, because `port-watch` resolves to an .exe before a .cmd.
#[cfg(target_os = "windows")]
const LEGACY_COPY_NAME: &str = "port-watch.exe";

/// How to spell the app's path inside a batch file. Batch files are read in
/// the console's legacy code page, so the text must be ASCII; a non-ASCII
/// part (in practice the user's profile folder) has to come from one of the
/// `env` variables instead. A literal `%` is doubled.
#[cfg(any(target_os = "windows", test))]
fn batch_path(exe: &str, env: &[(&str, String)]) -> Option<String> {
    let literal = |text: &str| text.replace('%', "%%");

    if exe.is_ascii() {
        return Some(literal(exe));
    }

    env.iter().find_map(|(name, value)| {
        let rest = exe
            .get(..value.len())
            .filter(|prefix| !value.is_empty() && prefix.eq_ignore_ascii_case(value))
            .map(|_| &exe[value.len()..])?;
        (rest.starts_with('\\') && rest.is_ascii()).then(|| format!("%{name}%{}", literal(rest)))
    })
}

// Delayed expansion is inherited from whoever runs the shim (`cmd /V:ON`, a
// script that enabled it) and would eat a `!` in the path or the arguments.
#[cfg(any(target_os = "windows", test))]
fn shim_contents(batch_path: &str) -> String {
    format!("@echo off\r\nsetlocal DisableDelayedExpansion\r\n\"{batch_path}\" %*\r\n")
}

#[cfg(target_os = "windows")]
fn shim_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Programs")
        .join("Port Watch")
}

#[cfg(target_os = "windows")]
fn expected_shim() -> Result<String, String> {
    let exe = current_app_executable()?;
    let env: Vec<(&str, String)> = ["LOCALAPPDATA", "APPDATA", "USERPROFILE"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok().map(|value| (name, value)))
        .collect();

    batch_path(&exe.to_string_lossy(), &env)
        .map(|path| shim_contents(&path))
        .ok_or_else(|| {
            format!(
                "{} contains characters a command-line shim cannot hold. Install the app in a folder with a plain name and try again.",
                exe.display()
            )
        })
}

#[cfg(target_os = "windows")]
pub fn cli_link_path() -> Result<String, String> {
    Ok(shim_dir().join(SHIM_NAME).to_string_lossy().into_owned())
}

#[cfg(target_os = "windows")]
pub fn get_cli_install_status() -> Result<CliInstallStatus, String> {
    Ok(status_in(&shim_dir(), expected_shim().ok().as_deref()))
}

// `expected` is the shim this app would write, when it can write one.
#[cfg(target_os = "windows")]
fn status_in(dir: &Path, expected: Option<&str>) -> CliInstallStatus {
    let shim = dir.join(SHIM_NAME);
    let legacy_copy = dir.join(LEGACY_COPY_NAME);
    let link_path = shim.to_string_lossy().into_owned();
    let has_legacy_copy = legacy_copy.exists();

    match std::fs::read_to_string(&shim) {
        Ok(contents) => CliInstallStatus {
            installed: true,
            target_path: Some(link_path.clone()),
            // A leftover copy still wins: `port-watch` finds the .exe first.
            points_to_app: !has_legacy_copy && expected == Some(contents.as_str()),
            link_path,
        },
        // Installed by an earlier version: on PATH, but not this app's shim.
        Err(_) => CliInstallStatus {
            installed: has_legacy_copy,
            link_path,
            target_path: has_legacy_copy.then(|| legacy_copy.to_string_lossy().into_owned()),
            points_to_app: false,
        },
    }
}

#[cfg(target_os = "windows")]
pub fn install_cli_to_path() -> Result<(), String> {
    let contents = expected_shim()?;
    let dir = shim_dir();

    write_shim_in(&dir, &contents)?;
    add_windows_cli_to_user_path(&dir)
}

// The old copy goes first: if it cannot be removed (still running, say) it
// would shadow the shim, so nothing is changed and the old CLI keeps working.
#[cfg(target_os = "windows")]
fn write_shim_in(dir: &Path, contents: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir)
        .map_err(|err| format!("Failed to create {}: {err}", dir.display()))?;
    remove_if_present(&dir.join(LEGACY_COPY_NAME))
        .map_err(|err| format!("Failed to remove the old CLI copy: {err}"))?;
    std::fs::write(dir.join(SHIM_NAME), contents)
        .map_err(|err| format!("Failed to write the CLI shim: {err}"))
}

#[cfg(target_os = "windows")]
pub fn uninstall_cli_from_path() -> Result<(), String> {
    let dir = shim_dir();
    if !dir.join(SHIM_NAME).exists() && !dir.join(LEGACY_COPY_NAME).exists() {
        return Ok(());
    }

    let _ = remove_windows_cli_from_user_path(&dir);
    remove_shim_from(&dir)
}

#[cfg(target_os = "windows")]
fn remove_shim_from(dir: &Path) -> Result<(), String> {
    remove_if_present(&dir.join(SHIM_NAME))
        .map_err(|err| format!("Failed to remove the CLI shim: {err}"))?;
    remove_if_present(&dir.join(LEGACY_COPY_NAME))
        .map_err(|err| format!("Failed to remove the old CLI copy: {err}"))
}

#[cfg(target_os = "windows")]
fn remove_if_present(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err),
        _ => Ok(()),
    }
}

#[cfg(target_os = "windows")]
fn powershell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(target_os = "windows")]
fn add_windows_cli_to_user_path(dir: &Path) -> Result<(), String> {
    let dir = powershell_single_quote(&dir.to_string_lossy());
    let script = format!(
        r#"$dir = {dir}
$current = [Environment]::GetEnvironmentVariable("Path", "User")
if ($null -eq $current) {{ $current = "" }}
$parts = $current -split ";" | Where-Object {{ $_ -and $_.Trim() -ne "" }}
if ($parts -notcontains $dir) {{
  $updated = if ($parts.Count -gt 0) {{ ($parts + $dir) -join ";" }} else {{ $dir }}
  [Environment]::SetEnvironmentVariable("Path", $updated, "User")
}}"#,
    );

    run_powershell(&script)
}

#[cfg(target_os = "windows")]
fn remove_windows_cli_from_user_path(dir: &Path) -> Result<(), String> {
    let dir = powershell_single_quote(&dir.to_string_lossy());
    let script = format!(
        r#"$dir = {dir}
$current = [Environment]::GetEnvironmentVariable("Path", "User")
if ($null -eq $current) {{ exit 0 }}
$parts = $current -split ";" | Where-Object {{ $_ -and $_.Trim() -ne "" -and $_.Trim() -ne $dir }}
$updated = $parts -join ";"
[Environment]::SetEnvironmentVariable("Path", $updated, "User")"#,
    );

    run_powershell(&script)
}

#[cfg(target_os = "windows")]
fn run_powershell(script: &str) -> Result<(), String> {
    use crate::platform::shell::NoWindow;
    let output = std::process::Command::new("powershell")
        .no_window()
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map_err(|err| format!("Failed to run PowerShell: {err}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Vec<(&'static str, String)> {
        vec![
            ("LOCALAPPDATA", r"C:\Users\José\AppData\Local".to_string()),
            ("USERPROFILE", r"C:\Users\José".to_string()),
        ]
    }

    #[test]
    fn batch_path_keeps_an_ascii_path_and_doubles_percent_signs() {
        assert_eq!(
            batch_path(r"C:\Program Files\Port Watch\port-watch.exe", &env()),
            Some(r"C:\Program Files\Port Watch\port-watch.exe".to_string())
        );
        assert_eq!(
            batch_path(r"D:\100% apps\port-watch.exe", &env()),
            Some(r"D:\100%% apps\port-watch.exe".to_string())
        );
    }

    #[test]
    fn batch_path_takes_a_non_ascii_profile_folder_from_the_environment() {
        assert_eq!(
            batch_path(
                r"C:\Users\José\AppData\Local\Port Watch\port-watch.exe",
                &env()
            ),
            Some(r"%LOCALAPPDATA%\Port Watch\port-watch.exe".to_string())
        );
        // Windows paths compare without regard to ASCII case.
        assert_eq!(
            batch_path(r"c:\users\José\Tools\50%\port-watch.exe", &env()),
            Some(r"%USERPROFILE%\Tools\50%%\port-watch.exe".to_string())
        );
    }

    #[test]
    fn batch_path_gives_up_when_no_variable_covers_the_non_ascii_part() {
        assert_eq!(
            batch_path(r"D:\Programme\Größe\port-watch.exe", &env()),
            None
        );
        assert_eq!(
            batch_path(r"C:\Users\José\Größe\port-watch.exe", &env()),
            None
        );
        // A sibling folder that merely starts with the variable's value.
        assert_eq!(batch_path(r"C:\Users\José2\port-watch.exe", &env()), None);
        assert_eq!(batch_path(r"D:\é\port-watch.exe", &[]), None);
    }

    #[test]
    fn shim_runs_the_app_with_every_argument() {
        assert_eq!(
            shim_contents(r"%LOCALAPPDATA%\Port Watch\port-watch.exe"),
            "@echo off\r\nsetlocal DisableDelayedExpansion\r\n\"%LOCALAPPDATA%\\Port Watch\\port-watch.exe\" %*\r\n"
        );
        assert!(SHIM_NAME.ends_with(".cmd"));
    }

    // The shim passes arguments through and hands back the exit code, which is
    // what `port-watch check` is for.
    #[test]
    #[cfg(target_os = "windows")]
    fn shim_passes_arguments_and_returns_the_exit_code() {
        let dir = tempfile::tempdir().unwrap();
        let shim = dir.path().join(SHIM_NAME);
        let cmd = std::env::var("ComSpec").unwrap();
        std::fs::write(&shim, shim_contents(&batch_path(&cmd, &[]).unwrap())).unwrap();

        let status = std::process::Command::new(&shim)
            .args(["/c", "exit 3"])
            .status()
            .unwrap();

        assert_eq!(status.code(), Some(3));
    }

    // A caller with delayed expansion on must not change what the shim runs.
    #[test]
    #[cfg(target_os = "windows")]
    fn shim_survives_delayed_expansion_in_the_caller() {
        let dir = tempfile::tempdir().unwrap();
        let odd = dir.path().join("Port!Watch");
        std::fs::create_dir(&odd).unwrap();
        let target = odd.join("exit-code.cmd");
        std::fs::write(&target, "@exit /b 7\r\n").unwrap();
        let shim = dir.path().join(SHIM_NAME);
        std::fs::write(
            &shim,
            shim_contents(&batch_path(&target.to_string_lossy(), &[]).unwrap()),
        )
        .unwrap();

        let status = std::process::Command::new(std::env::var("ComSpec").unwrap())
            .args(["/V:ON", "/C"])
            .arg(&shim)
            .status()
            .unwrap();

        assert_eq!(status.code(), Some(7));
    }

    #[cfg(target_os = "windows")]
    mod windows_shim {
        use super::super::{
            remove_shim_from, status_in, write_shim_in, LEGACY_COPY_NAME, SHIM_NAME,
        };
        use std::fs;
        use std::os::windows::fs::OpenOptionsExt;

        const SHIM: &str = "@echo off\r\n\"C:\\App\\port-watch.exe\" %*\r\n";

        #[test]
        fn installing_replaces_the_copy_an_earlier_version_left() {
            let dir = tempfile::tempdir().unwrap();
            fs::write(dir.path().join(LEGACY_COPY_NAME), "old").unwrap();
            let before = status_in(dir.path(), Some(SHIM));
            assert!(before.installed && !before.points_to_app);

            write_shim_in(dir.path(), SHIM).unwrap();

            assert!(!dir.path().join(LEGACY_COPY_NAME).exists());
            assert_eq!(
                fs::read_to_string(dir.path().join(SHIM_NAME)).unwrap(),
                SHIM
            );
            let after = status_in(dir.path(), Some(SHIM));
            assert!(after.installed && after.points_to_app);
        }

        // `port-watch` resolves to the .exe before the .cmd.
        #[test]
        fn a_leftover_copy_means_the_shim_is_not_what_runs() {
            let dir = tempfile::tempdir().unwrap();
            fs::write(dir.path().join(SHIM_NAME), SHIM).unwrap();
            fs::write(dir.path().join(LEGACY_COPY_NAME), "old").unwrap();

            let status = status_in(dir.path(), Some(SHIM));
            assert!(status.installed && !status.points_to_app);
        }

        #[test]
        fn a_shim_for_another_install_is_not_this_app() {
            let dir = tempfile::tempdir().unwrap();
            fs::write(
                dir.path().join(SHIM_NAME),
                "@echo off\r\n\"D:\\Other\\port-watch.exe\" %*\r\n",
            )
            .unwrap();

            let status = status_in(dir.path(), Some(SHIM));
            assert!(status.installed && !status.points_to_app);
            assert!(!status_in(dir.path(), None).points_to_app);
        }

        #[test]
        fn nothing_changes_when_the_old_copy_cannot_be_removed() {
            let dir = tempfile::tempdir().unwrap();
            let legacy = dir.path().join(LEGACY_COPY_NAME);
            fs::write(&legacy, "old").unwrap();
            // Held open without delete sharing, as a running program would be.
            let in_use = fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(&legacy)
                .unwrap();

            let err = write_shim_in(dir.path(), SHIM).unwrap_err();
            drop(in_use);

            assert!(err.contains("old CLI copy"), "{err}");
            assert!(legacy.exists());
            assert!(!dir.path().join(SHIM_NAME).exists());
        }

        #[test]
        fn uninstalling_removes_the_shim_and_any_old_copy() {
            let dir = tempfile::tempdir().unwrap();
            fs::write(dir.path().join(SHIM_NAME), SHIM).unwrap();
            fs::write(dir.path().join(LEGACY_COPY_NAME), "old").unwrap();

            remove_shim_from(dir.path()).unwrap();

            assert!(!status_in(dir.path(), Some(SHIM)).installed);
            assert_eq!(remove_shim_from(dir.path()), Ok(()));
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn shell_escape_handles_single_quotes() {
        assert_eq!(
            shell_escape("/Applications/Port Watch.app/Contents/MacOS/port-watch"),
            "'/Applications/Port Watch.app/Contents/MacOS/port-watch'"
        );
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn admin_script_creates_the_folder_and_replaces_the_link() {
        let source = Path::new("/Applications/Port Watch.app/Contents/MacOS/port-watch");
        let link = Path::new("/usr/local/bin/port-watch");
        assert_eq!(
            admin_shell_script(unix::PrivilegedStep::Link { source, link }),
            "mkdir -p '/usr/local/bin' && ln -sf '/Applications/Port Watch.app/Contents/MacOS/port-watch' '/usr/local/bin/port-watch'"
        );
        assert_eq!(
            admin_shell_script(unix::PrivilegedStep::Unlink { link }),
            "rm -f '/usr/local/bin/port-watch'"
        );
    }

    #[cfg(unix)]
    mod unix_links {
        use super::super::unix::{install, status, uninstall, PrivilegedStep};
        use std::fs;
        use std::os::unix::fs::{symlink, PermissionsExt};
        use std::path::{Path, PathBuf};
        use std::sync::Mutex;

        struct Fixture {
            dir: tempfile::TempDir,
            app: PathBuf,
        }

        impl Fixture {
            fn new() -> Self {
                let dir = tempfile::tempdir().unwrap();
                let app = dir.path().join("Port Watch.app/port-watch");
                fs::create_dir_all(app.parent().unwrap()).unwrap();
                fs::write(&app, "").unwrap();
                Self { dir, app }
            }

            fn path(&self, relative: &str) -> PathBuf {
                self.dir.path().join(relative)
            }

            // A folder the test cannot write to, or None when running as root,
            // which ignores permissions.
            fn read_only(&self, relative: &str) -> Option<PathBuf> {
                let folder = self.path(relative);
                fs::create_dir_all(&folder).unwrap();
                fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).unwrap();
                fs::write(folder.join("probe"), "")
                    .is_err()
                    .then_some(folder)
            }
        }

        // Writable again, so the temporary directory can be cleaned up.
        impl Drop for Fixture {
            fn drop(&mut self) {
                for relative in ["usr-local", "bin"] {
                    let _ =
                        fs::set_permissions(self.path(relative), fs::Permissions::from_mode(0o755));
                }
            }
        }

        fn must_not_escalate(step: PrivilegedStep) -> Result<(), String> {
            panic!("unexpected escalation: {step:?}");
        }

        // Records the step it was asked for as text.
        fn recorder(
            log: &Mutex<Vec<String>>,
        ) -> impl Fn(PrivilegedStep) -> Result<(), String> + Sync + '_ {
            |step| {
                log.lock().unwrap().push(format!("{step:?}"));
                Ok(())
            }
        }

        #[test]
        fn installs_into_a_folder_that_does_not_exist_yet() {
            let fixture = Fixture::new();
            let link = fixture.path("local/bin/port-watch");

            assert_eq!(
                install(&link, &fixture.app, Some(&must_not_escalate)),
                Ok(())
            );

            assert_eq!(fs::read_link(&link).unwrap(), fixture.app);
            let status = status(&link, &fixture.app);
            assert!(status.installed && status.points_to_app);
            // Again: already in place.
            assert_eq!(
                install(&link, &fixture.app, Some(&must_not_escalate)),
                Ok(())
            );
        }

        #[test]
        fn replaces_a_dangling_link() {
            let fixture = Fixture::new();
            let link = fixture.path("bin/port-watch");
            fs::create_dir_all(link.parent().unwrap()).unwrap();
            symlink(fixture.path("old-location/port-watch"), &link).unwrap();

            let before = status(&link, &fixture.app);
            assert!(before.installed && !before.points_to_app);

            assert_eq!(install(&link, &fixture.app, None), Ok(()));
            assert_eq!(fs::read_link(&link).unwrap(), fixture.app);
        }

        #[test]
        fn leaves_someone_elses_install_alone() {
            let fixture = Fixture::new();
            let link = fixture.path("bin/port-watch");
            let other = fixture.path("other-port-watch");
            fs::create_dir_all(link.parent().unwrap()).unwrap();
            fs::write(&other, "").unwrap();
            symlink(&other, &link).unwrap();

            let err = install(&link, &fixture.app, Some(&must_not_escalate)).unwrap_err();
            assert!(err.contains("Another port-watch is installed"), "{err}");
            let err = uninstall(&link, &fixture.app, Some(&must_not_escalate)).unwrap_err();
            assert!(err.contains("not this app"), "{err}");
            assert_eq!(fs::read_link(&link).unwrap(), other);
        }

        #[test]
        fn leaves_a_regular_file_alone() {
            let fixture = Fixture::new();
            let link = fixture.path("bin/port-watch");
            fs::create_dir_all(link.parent().unwrap()).unwrap();
            fs::write(&link, "#!/bin/sh\n").unwrap();

            let found = status(&link, &fixture.app);
            assert!(found.installed && !found.points_to_app);
            let err = install(&link, &fixture.app, Some(&must_not_escalate)).unwrap_err();
            assert!(err.contains("not a symlink"), "{err}");
            let err = uninstall(&link, &fixture.app, Some(&must_not_escalate)).unwrap_err();
            assert!(err.contains("not a symlink"), "{err}");
            assert!(link.is_file());
        }

        #[test]
        fn uninstall_removes_its_own_and_dangling_links() {
            let fixture = Fixture::new();
            let link = fixture.path("bin/port-watch");

            assert_eq!(uninstall(&link, &fixture.app, None), Ok(()));

            install(&link, &fixture.app, None).unwrap();
            assert_eq!(uninstall(&link, &fixture.app, None), Ok(()));
            assert!(!status(&link, &fixture.app).installed);

            symlink(fixture.path("gone/port-watch"), &link).unwrap();
            assert_eq!(uninstall(&link, &fixture.app, None), Ok(()));
            assert!(!status(&link, &fixture.app).installed);
        }

        // /usr/local/bin missing under a root-owned /usr/local: creating the
        // folder is what fails, before any link is attempted.
        #[test]
        fn asks_for_privileges_when_the_folder_cannot_be_created() {
            let fixture = Fixture::new();
            let Some(parent) = fixture.read_only("usr-local") else {
                return;
            };
            let link = parent.join("bin/port-watch");
            let log = Mutex::new(Vec::new());

            assert_eq!(install(&link, &fixture.app, Some(&recorder(&log))), Ok(()));

            let expected = PrivilegedStep::Link {
                source: &fixture.app,
                link: &link,
            };
            assert_eq!(*log.lock().unwrap(), vec![format!("{expected:?}")]);
        }

        #[test]
        fn asks_for_privileges_when_the_folder_is_not_writable() {
            let fixture = Fixture::new();
            let Some(folder) = fixture.read_only("bin") else {
                return;
            };
            let link = folder.join("port-watch");
            let log = Mutex::new(Vec::new());

            assert_eq!(install(&link, &fixture.app, Some(&recorder(&log))), Ok(()));
            assert_eq!(log.lock().unwrap().len(), 1);
        }

        #[test]
        fn asks_for_privileges_to_replace_or_remove_a_link_it_cannot_touch() {
            let fixture = Fixture::new();
            let folder = fixture.path("bin");
            fs::create_dir_all(&folder).unwrap();
            let dangling = folder.join("port-watch");
            symlink(fixture.path("gone/port-watch"), &dangling).unwrap();
            let Some(folder) = fixture.read_only("bin") else {
                return;
            };
            let link = folder.join("port-watch");
            let log = Mutex::new(Vec::new());

            assert_eq!(install(&link, &fixture.app, Some(&recorder(&log))), Ok(()));
            assert_eq!(
                uninstall(&link, &fixture.app, Some(&recorder(&log))),
                Ok(())
            );

            let relink = PrivilegedStep::Link {
                source: &fixture.app,
                link: &link,
            };
            let unlink = PrivilegedStep::Unlink { link: &link };
            assert_eq!(
                *log.lock().unwrap(),
                vec![format!("{relink:?}"), format!("{unlink:?}")]
            );
        }

        #[test]
        fn without_a_way_to_escalate_a_permission_error_is_reported() {
            let fixture = Fixture::new();
            let Some(folder) = fixture.read_only("bin") else {
                return;
            };

            let err = install(&folder.join("port-watch"), &fixture.app, None).unwrap_err();
            assert!(err.starts_with("Failed to create the CLI link"), "{err}");
        }

        #[test]
        fn a_refused_escalation_is_reported() {
            let fixture = Fixture::new();
            let Some(folder) = fixture.read_only("bin") else {
                return;
            };
            let refuse = |_: PrivilegedStep| Err::<(), _>("User canceled.".to_string());

            let err = install(&folder.join("port-watch"), &fixture.app, Some(&refuse)).unwrap_err();
            assert_eq!(err, "User canceled.");
            assert!(!Path::new(&folder.join("port-watch")).exists());
        }
    }
}
