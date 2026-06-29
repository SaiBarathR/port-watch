use std::sync::Mutex;

use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, Wry,
};

use crate::app_settings::AppSettings;
use crate::poller::PortPoller;
use crate::scanner::PortProcess;

pub struct TrayState {
    pub user_listener_count: u32,
    pub menu_bar_mode_enabled: bool,
    pub last_menu_signature: Option<String>,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            user_listener_count: 0,
            menu_bar_mode_enabled: false,
            last_menu_signature: None,
        }
    }
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(Mutex::new(TrayState::default()));

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("Missing application icon for tray")?;

    // Initial menu (no scan yet) — the poller rebuilds it as soon as it has data.
    let menu = build_menu(app, &[], false, false)?;

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("Port Watch")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| handle_menu_event(app, event.id.as_ref()))
        .build(app)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Menu construction
// ---------------------------------------------------------------------------

fn primary_port(process: &PortProcess) -> Option<u16> {
    process.ports.first().map(|binding| binding.port)
}

/// Directory used by the Finder/Terminal/Editor actions. Matches the main
/// window's table actions, which prefer `project_root` over `working_directory`
/// (see `port-table-actions-cell.tsx`).
fn directory_for(process: &PortProcess) -> String {
    if !process.project_root.is_empty() {
        process.project_root.clone()
    } else {
        process.working_directory.clone()
    }
}

fn localhost_url(port: u16, use_https: bool) -> String {
    let scheme = if use_https { "https" } else { "http" };
    format!("{scheme}://localhost:{port}")
}

fn build_port_submenu(
    app: &AppHandle,
    process: &PortProcess,
    allow_system: bool,
) -> tauri::Result<Submenu<Wry>> {
    let pid = process.pid;
    let port = primary_port(process);
    let has_dir = !directory_for(process).is_empty();
    let can_stop = !process.is_system_service || allow_system;

    let title = match port {
        Some(p) => format!("{p}  ·  {}", process.name),
        None => process.name.clone(),
    };

    let open = MenuItem::with_id(
        app,
        format!("pw-open:{pid}:{}", port.unwrap_or(0)),
        match port {
            Some(p) => format!("Open localhost:{p}"),
            None => "Open in browser".to_string(),
        },
        port.is_some(),
        None::<&str>,
    )?;
    let copy = MenuItem::with_id(
        app,
        format!("pw-copy:{pid}:{}", port.unwrap_or(0)),
        "Copy URL",
        port.is_some(),
        None::<&str>,
    )?;
    let finder = MenuItem::with_id(
        app,
        format!("pw-finder:{pid}"),
        "Show in Finder",
        has_dir,
        None::<&str>,
    )?;
    let terminal = MenuItem::with_id(
        app,
        format!("pw-terminal:{pid}"),
        "Open in Terminal",
        has_dir,
        None::<&str>,
    )?;
    let editor = MenuItem::with_id(
        app,
        format!("pw-editor:{pid}"),
        "Open in Editor",
        has_dir,
        None::<&str>,
    )?;
    let stop = MenuItem::with_id(
        app,
        format!("pw-stop:{pid}"),
        "Stop process",
        can_stop,
        None::<&str>,
    )?;
    let sep_open = PredefinedMenuItem::separator(app)?;
    let sep_stop = PredefinedMenuItem::separator(app)?;

    Submenu::with_items(
        app,
        title,
        true,
        &[
            &open, &copy, &sep_open, &finder, &terminal, &editor, &sep_stop, &stop,
        ],
    )
}

fn build_menu(
    app: &AppHandle,
    processes: &[PortProcess],
    allow_system: bool,
    menu_bar_enabled: bool,
) -> tauri::Result<Menu<Wry>> {
    let mut user: Vec<&PortProcess> = processes
        .iter()
        .filter(|process| !process.is_system_service)
        .collect();
    user.sort_by_key(|process| primary_port(process).unwrap_or(u16::MAX));

    let count = user.len();
    let header_label = if count == 1 {
        "Port Watch — 1 listener".to_string()
    } else {
        format!("Port Watch — {count} listeners")
    };
    let header = MenuItem::with_id(app, "pw-header", header_label, false, None::<&str>)?;

    let menu = Menu::new(app)?;
    menu.append(&header)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    if user.is_empty() {
        let empty = MenuItem::with_id(
            app,
            "pw-empty",
            "No dev servers listening",
            false,
            None::<&str>,
        )?;
        menu.append(&empty)?;
    } else {
        for process in &user {
            let submenu = build_port_submenu(app, process, allow_system)?;
            menu.append(&submenu)?;
        }
    }

    menu.append(&PredefinedMenuItem::separator(app)?)?;

    let open_window = MenuItem::with_id(
        app,
        "tray-open-window",
        "Open Full Window",
        true,
        None::<&str>,
    )?;
    let refresh = MenuItem::with_id(app, "tray-refresh", "Refresh", true, None::<&str>)?;
    let menu_bar_mode = CheckMenuItem::with_id(
        app,
        "tray-menu-bar-mode",
        "Menu bar mode",
        true,
        menu_bar_enabled,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "tray-quit", "Quit", true, None::<&str>)?;

    menu.append(&open_window)?;
    menu.append(&refresh)?;
    menu.append(&menu_bar_mode)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&quit)?;

    Ok(menu)
}

fn menu_signature(processes: &[PortProcess], allow_system: bool, menu_bar_enabled: bool) -> String {
    let mut parts: Vec<String> = processes
        .iter()
        .filter(|process| !process.is_system_service)
        .map(|process| {
            let mut bindings: Vec<String> = process
                .ports
                .iter()
                .map(|binding| format!("{}:{}/{}", binding.address, binding.port, binding.protocol))
                .collect();
            bindings.sort();
            format!(
                "{}|{}|{}|{}",
                process.pid,
                process.name,
                bindings.join(","),
                directory_for(process)
            )
        })
        .collect();
    parts.sort();
    format!(
        "{}|allow={}|mbm={}",
        parts.join(";"),
        allow_system,
        menu_bar_enabled
    )
}

/// Rebuild the native tray menu from the poller's latest scan. Cheap to call on
/// every scan — it diffs a signature and only touches the menu when something
/// the menu shows actually changed. The menu must be mutated on the main thread.
pub fn rebuild_tray_menu(app: &AppHandle) {
    let processes = app.state::<PortPoller>().snapshot();
    let allow_system = app.state::<AppSettings>().allow_system_process_actions();
    let menu_bar_enabled = is_menu_bar_mode_enabled(app);

    let signature = menu_signature(&processes, allow_system, menu_bar_enabled);
    if let Some(state) = app.try_state::<Mutex<TrayState>>() {
        let mut guard = match state.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        if guard.last_menu_signature.as_deref() == Some(signature.as_str()) {
            return;
        }
        guard.last_menu_signature = Some(signature);
        guard.user_listener_count = processes
            .iter()
            .filter(|process| !process.is_system_service)
            .count() as u32;
    }

    let user_count = processes
        .iter()
        .filter(|process| !process.is_system_service)
        .count() as u32;

    let app_main = app.clone();
    let _ = app.run_on_main_thread(move || {
        match build_menu(&app_main, &processes, allow_system, menu_bar_enabled) {
            Ok(menu) => {
                if let Some(tray) = app_main.tray_by_id("main") {
                    let _ = tray.set_menu(Some(menu));
                    let label = if user_count == 1 {
                        "1 listener".to_string()
                    } else {
                        format!("{user_count} listeners")
                    };
                    let _ = tray.set_tooltip(Some(&label));
                }
            }
            Err(error) => eprintln!("Failed to build tray menu: {error}"),
        }
    });
}

// ---------------------------------------------------------------------------
// Menu events
// ---------------------------------------------------------------------------

fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        "tray-open-window" => show_full_window(app),
        "tray-refresh" => {
            let _ = crate::poller::trigger_port_scan(app.clone());
        }
        "tray-menu-bar-mode" => {
            let enabled = !is_menu_bar_mode_enabled(app);
            if let Err(error) = apply_menu_bar_mode(app, enabled) {
                eprintln!("Failed to apply menu bar mode: {error}");
            }
        }
        "tray-quit" => app.exit(0),
        other => handle_port_action(app, other),
    }
}

fn handle_port_action(app: &AppHandle, id: &str) {
    let Some((action, rest)) = id.split_once(':') else {
        return;
    };
    if !action.starts_with("pw-") {
        return;
    }

    let mut fields = rest.split(':');
    let Some(pid) = fields.next().and_then(|value| value.parse::<u32>().ok()) else {
        return;
    };
    let port = fields.next().and_then(|value| value.parse::<u16>().ok());

    let process = app.state::<PortPoller>().find_by_pid(pid);
    let app = app.clone();
    let action = action.to_string();

    // Run off the main thread: stop_process can block up to 2s and the launch
    // helpers wait on a child process — neither should freeze the UI thread.
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = run_port_action(&app, &action, pid, port, process.as_ref()) {
            notify_error(&app, &error);
        }
    });
}

fn run_port_action(
    app: &AppHandle,
    action: &str,
    pid: u32,
    port: Option<u16>,
    process: Option<&PortProcess>,
) -> Result<(), String> {
    match action {
        "pw-open" => {
            let port = port.ok_or("No port available")?;
            let use_https = app.state::<AppSettings>().use_https_for_localhost();
            crate::commands::workflow::open_url(app.clone(), localhost_url(port, use_https))
        }
        "pw-copy" => {
            let port = port.ok_or("No port available")?;
            let use_https = app.state::<AppSettings>().use_https_for_localhost();
            crate::platform::shell::copy_to_clipboard(&localhost_url(port, use_https))
        }
        "pw-finder" => {
            crate::commands::filesystem::open_in_finder(require_directory(process)?)
        }
        "pw-terminal" => {
            crate::commands::workflow::open_in_terminal(require_directory(process)?)
        }
        "pw-editor" => {
            let editor = app.state::<AppSettings>().preferred_editor();
            crate::commands::workflow::open_in_editor(require_directory(process)?, editor)
        }
        "pw-stop" => {
            if !confirm_stop(app, process) {
                return Ok(());
            }
            crate::commands::process::stop_process(app.clone(), pid, Some(false))
        }
        _ => Ok(()),
    }
}

/// Native confirmation before terminating a process — restores the safety the
/// removed popover's StopDialog provided. Runs on a background thread (the
/// caller is `spawn_blocking`), so blocking on the user's response is fine.
fn confirm_stop(app: &AppHandle, process: Option<&PortProcess>) -> bool {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

    let name = process.map(|p| p.name.as_str()).unwrap_or("this process");
    let is_system = process.map(|p| p.is_system_service).unwrap_or(false);
    let message = if is_system {
        format!("{name} is a system service. Stopping it may affect your system.\n\nStop it anyway?")
    } else {
        format!("Stop {name}? This terminates the process and frees its ports.")
    };

    app.dialog()
        .message(message)
        .title("Stop process")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Stop".to_string(),
            "Cancel".to_string(),
        ))
        .blocking_show()
}

fn require_directory(process: Option<&PortProcess>) -> Result<String, String> {
    process
        .map(directory_for)
        .filter(|dir| !dir.is_empty())
        .ok_or_else(|| "No folder available for this process".to_string())
}

fn notify_error(app: &AppHandle, message: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app
        .notification()
        .builder()
        .title("Port Watch")
        .body(message)
        .show();
    eprintln!("Tray action failed: {message}");
}

// ---------------------------------------------------------------------------
// Window / menu-bar-mode helpers
// ---------------------------------------------------------------------------

fn is_menu_bar_mode_enabled(app: &AppHandle) -> bool {
    app.try_state::<Mutex<TrayState>>()
        .and_then(|state| state.lock().ok().map(|guard| guard.menu_bar_mode_enabled))
        .unwrap_or(false)
}

fn set_menu_bar_mode_state(app: &AppHandle, enabled: bool) {
    if let Some(state) = app.try_state::<Mutex<TrayState>>() {
        if let Ok(mut guard) = state.lock() {
            guard.menu_bar_mode_enabled = enabled;
        }
    }
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn show_full_window(app: &AppHandle) {
    show_main_window(app);
}

fn apply_menu_bar_mode(app: &AppHandle, enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let policy = if enabled {
            tauri::ActivationPolicy::Accessory
        } else {
            tauri::ActivationPolicy::Regular
        };
        app.set_activation_policy(policy)
            .map_err(|e| format!("Failed to set activation policy: {e}"))?;
    }

    if enabled {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
    } else {
        show_main_window(app);
    }

    set_menu_bar_mode_state(app, enabled);
    let _ = app.emit("tray-menu-bar-mode-changed", enabled);
    rebuild_tray_menu(app);
    Ok(())
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn update_tray_count(app: AppHandle, user_count: u32) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("main") {
        let label = if user_count == 1 {
            "1 listener".to_string()
        } else {
            format!("{user_count} listeners")
        };
        let _ = tray.set_tooltip(Some(&label));
    }

    if let Ok(mut state) = app.state::<Mutex<TrayState>>().lock() {
        state.user_listener_count = user_count;
    }

    Ok(())
}

#[tauri::command]
pub fn set_menu_bar_mode(app: AppHandle, enabled: bool) -> Result<(), String> {
    apply_menu_bar_mode(&app, enabled)
}

#[tauri::command]
pub fn show_full_window_command(app: AppHandle) -> Result<(), String> {
    show_full_window(&app);
    Ok(())
}
