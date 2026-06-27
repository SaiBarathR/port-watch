use crate::cli_install::{self, CliInstallStatus};

#[tauri::command]
pub fn get_cli_install_status() -> Result<CliInstallStatus, String> {
    cli_install::get_cli_install_status()
}

#[tauri::command]
pub fn install_cli_to_path() -> Result<(), String> {
    cli_install::install_cli_to_path()
}

#[tauri::command]
pub fn uninstall_cli_from_path() -> Result<(), String> {
    cli_install::uninstall_cli_from_path()
}
