use crate::cli_install::{self, CliInstallStatus};

#[tauri::command]
pub fn get_cli_install_status() -> Result<CliInstallStatus, String> {
    cli_install::get_cli_install_status()
}

// Async: on macOS these can sit behind the administrator password prompt, and
// on Windows they run PowerShell. Neither should hold the main thread.
#[tauri::command]
pub async fn install_cli_to_path() -> Result<(), String> {
    super::blocking("Install", cli_install::install_cli_to_path).await
}

#[tauri::command]
pub async fn uninstall_cli_from_path() -> Result<(), String> {
    super::blocking("Uninstall", cli_install::uninstall_cli_from_path).await
}
