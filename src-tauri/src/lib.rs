mod app_settings;
mod classifier;
pub mod cli;
pub mod cli_install;
pub mod commands;
mod guards;
mod home;
mod platform;
mod poller;
mod process_actions;
pub mod scanner;
mod tray;

use app_settings::AppSettings;
use commands::cli_install::{
    get_cli_install_status, install_cli_to_path, uninstall_cli_from_path,
};
use commands::filesystem::{delete_permanently, move_to_trash, open_in_finder};
use commands::notifications::send_notification;
use commands::ports::list_listening_ports;
use commands::process::stop_process;
use commands::settings::set_allow_system_process_actions;
use commands::workflow::{open_in_editor, open_in_terminal, open_url};
use poller::{
    get_listening_ports, set_refresh_paused, set_scan_settings, start_poller, trigger_port_scan,
    PortPoller,
};
use tray::{
    set_menu_bar_mode, setup_tray, show_full_window_command, update_tray_count,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_os::init());

    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_plugin_liquid_glass::init());
    }

    builder
        .manage(PortPoller::new())
        .manage(AppSettings::new())
        .setup(|app| {
            setup_tray(app.handle())?;
            start_poller(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_listening_ports,
            get_listening_ports,
            set_scan_settings,
            set_refresh_paused,
            trigger_port_scan,
            set_allow_system_process_actions,
            stop_process,
            open_in_finder,
            move_to_trash,
            delete_permanently,
            open_url,
            open_in_terminal,
            open_in_editor,
            update_tray_count,
            set_menu_bar_mode,
            show_full_window_command,
            send_notification,
            get_cli_install_status,
            install_cli_to_path,
            uninstall_cli_from_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
