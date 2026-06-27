use std::path::{Path, PathBuf};
use std::process;

use serde::Serialize;

pub const CLI_LINK_PATH: &str = "/usr/local/bin/port-watch";

#[derive(Debug, Clone, Serialize)]
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
            println!("Installed {CLI_LINK_PATH}");
            process::exit(0);
        }
        Err(err) => {
            eprintln!("{err}");
            process::exit(1);
        }
    }
}

pub fn get_cli_install_status() -> Result<CliInstallStatus, String> {
    platform_get_cli_install_status()
}

pub fn install_cli_to_path() -> Result<(), String> {
    platform_install_cli_to_path()
}

pub fn uninstall_cli_from_path() -> Result<(), String> {
    platform_uninstall_cli_from_path()
}

#[cfg(target_os = "macos")]
fn platform_get_cli_install_status() -> Result<CliInstallStatus, String> {
    let app_exe = current_app_executable()?;
    let link_path = PathBuf::from(CLI_LINK_PATH);

    if !link_path.exists() {
        return Ok(CliInstallStatus {
            installed: false,
            link_path: CLI_LINK_PATH.to_string(),
            target_path: None,
            points_to_app: false,
        });
    }

    let target_path = read_link_target(&link_path);
    let points_to_app = target_path
        .as_ref()
        .is_some_and(|target| paths_refer_to_same_file(target, &app_exe));

    Ok(CliInstallStatus {
        installed: true,
        link_path: CLI_LINK_PATH.to_string(),
        target_path,
        points_to_app,
    })
}

#[cfg(target_os = "macos")]
fn platform_install_cli_to_path() -> Result<(), String> {
    let app_exe = current_app_executable()?;
    let link_path = PathBuf::from(CLI_LINK_PATH);

    if link_path.exists() {
        if let Some(target) = read_link_target(&link_path) {
            if paths_refer_to_same_file(&target, &app_exe) {
                return Ok(());
            }
            return Err(format!(
                "Another port-watch is installed at {CLI_LINK_PATH} (points to {target})"
            ));
        }
        return Err(format!(
            "{CLI_LINK_PATH} exists but is not a symlink. Remove it manually and try again."
        ));
    }

    if let Some(parent) = link_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|err| {
                format!("Failed to create {}: {err}", parent.display())
            })?;
        }
    }

    match std::os::unix::fs::symlink(&app_exe, &link_path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            install_with_admin_privileges(&app_exe)
        }
        Err(err) => Err(format!("Failed to install CLI: {err}")),
    }
}

#[cfg(target_os = "macos")]
fn platform_uninstall_cli_from_path() -> Result<(), String> {
    let app_exe = current_app_executable()?;
    let link_path = PathBuf::from(CLI_LINK_PATH);

    if !link_path.exists() {
        return Ok(());
    }

    let Some(target) = read_link_target(&link_path) else {
        return Err(format!(
            "{CLI_LINK_PATH} exists but is not a symlink. Remove it manually."
        ));
    };

    if !paths_refer_to_same_file(&target, &app_exe) {
        return Err(format!(
            "{CLI_LINK_PATH} points to {target}, not this app. Uninstall skipped."
        ));
    }

    std::fs::remove_file(&link_path).map_err(|err| format!("Failed to remove CLI link: {err}"))
}

#[cfg(target_os = "macos")]
fn current_app_executable() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|err| format!("Failed to resolve app executable: {err}"))
}

#[cfg(target_os = "macos")]
fn read_link_target(path: &Path) -> Option<String> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_symlink() {
        return None;
    }

    std::fs::read_link(path)
        .ok()
        .map(|target| target.to_string_lossy().into_owned())
}

#[cfg(target_os = "macos")]
fn canonicalize_if_exists(path: &str) -> Option<PathBuf> {
    let path = PathBuf::from(path);
    if path.exists() {
        std::fs::canonicalize(path).ok()
    } else {
        Some(path)
    }
}

#[cfg(target_os = "macos")]
fn paths_refer_to_same_file(left: &str, right: &Path) -> bool {
    match (canonicalize_if_exists(left), canonicalize_if_exists(&right.to_string_lossy())) {
        (Some(left_path), Some(right_path)) => left_path == right_path,
        _ => PathBuf::from(left) == right,
    }
}

#[cfg(target_os = "macos")]
fn install_with_admin_privileges(source: &Path) -> Result<(), String> {
    let source = source.to_string_lossy();
    let script = format!(
        "mkdir -p /usr/local/bin && ln -sf {} {}",
        shell_escape(&source),
        shell_escape(CLI_LINK_PATH)
    );
    let osa_script = format!(
        "do shell script {} with administrator privileges",
        applescript_string(&script)
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
            Err("Installation was cancelled or failed.".to_string())
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

#[cfg(not(target_os = "macos"))]
fn platform_get_cli_install_status() -> Result<CliInstallStatus, String> {
    Err("CLI install is only supported on macOS".to_string())
}

#[cfg(not(target_os = "macos"))]
fn platform_install_cli_to_path() -> Result<(), String> {
    Err("CLI install is only supported on macOS".to_string())
}

#[cfg(not(target_os = "macos"))]
fn platform_uninstall_cli_from_path() -> Result<(), String> {
    Err("CLI install is only supported on macOS".to_string())
}

#[cfg(test)]
#[cfg(target_os = "macos")]
mod tests {
    use super::*;

    #[test]
    fn shell_escape_handles_single_quotes() {
        assert_eq!(
            shell_escape("/Applications/Port Watch.app/Contents/MacOS/port-watch"),
            "'/Applications/Port Watch.app/Contents/MacOS/port-watch'"
        );
        assert_eq!(shell_escape("it's"), "'it'\\''s'");
    }
}
