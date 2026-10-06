use std::sync::Mutex;

use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, Wry,
};

use crate::app_settings::AppSettings;
use crate::poller::PortPoller;
use crate::scanner::PortProcess;

#[derive(Default)]
pub struct TrayState {
    pub menu_bar_mode_enabled: bool,
    pub last_menu_signature: Option<String>,
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(Mutex::new(TrayState::default()));

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("Missing application icon for tray")?;

    // Initial menu (no scan yet) — the poller rebuilds it as soon as it has data.
    let menu = build_menu(app, &[], false)?;

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

fn localhost_url(port: u16, use_https: bool) -> String {
    let scheme = if use_https { "https" } else { "http" };
    format!("{scheme}://localhost:{port}")
}

fn build_port_submenu(app: &AppHandle, process: &PortProcess) -> tauri::Result<Submenu<Wry>> {
    let pid = process.pid;
    let port = primary_port(process);
    let has_dir = !process.project_dir().is_empty();

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
        true,
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
            let submenu = build_port_submenu(app, process)?;
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

fn menu_signature(processes: &[PortProcess], menu_bar_enabled: bool) -> String {
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
                process.project_dir()
            )
        })
        .collect();
    parts.sort();
    format!("{}|mbm={}", parts.join(";"), menu_bar_enabled)
}

/// Rebuild the native tray menu from the poller's latest scan. Cheap to call on
/// every scan — it diffs a signature and only touches the menu when something
/// the menu shows actually changed. The menu must be mutated on the main thread.
pub fn rebuild_tray_menu(app: &AppHandle) {
    let processes = app.state::<PortPoller>().processes();
    let menu_bar_enabled = is_menu_bar_mode_enabled(app);

    let signature = menu_signature(&processes, menu_bar_enabled);
    if let Some(state) = app.try_state::<Mutex<TrayState>>() {
        let guard = match state.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        if guard.last_menu_signature.as_deref() == Some(signature.as_str()) {
            return;
        }
    }

    let user_count = processes
        .iter()
        .filter(|process| !process.is_system_service)
        .count() as u32;

    let app_main = app.clone();
    let _ = app.run_on_main_thread(move || {
        match build_menu(&app_main, &processes, menu_bar_enabled) {
            Ok(menu) => {
                if let Some(tray) = app_main.tray_by_id("main") {
                    let _ = tray.set_menu(Some(menu));
                    let label = if user_count == 1 {
                        "1 listener".to_string()
                    } else {
                        format!("{user_count} listeners")
                    };
                    let _ = tray.set_tooltip(Some(&label));

                    // Commit the signature only after the menu is actually
                    // applied — a failed build/dispatch must retry on the next
                    // scan instead of being recorded as "up to date". Serialized
                    // on the main thread, so the stored signature always matches
                    // the last applied menu.
                    if let Some(state) = app_main.try_state::<Mutex<TrayState>>() {
                        if let Ok(mut guard) = state.lock() {
                            guard.last_menu_signature = Some(signature);
                        }
                    }
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
        "tray-open-window" => show_main_window(app),
        "tray-refresh" => app.state::<PortPoller>().request_scan(),
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

    // Run off the main thread: stop_process can block for seconds and the launch
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
            crate::commands::filesystem::open_in_finder_blocking(&require_directory(process)?)
        }
        "pw-terminal" => {
            crate::commands::workflow::open_in_terminal_blocking(&require_directory(process)?)
        }
        "pw-editor" => {
            let editor = app.state::<AppSettings>().preferred_editor();
            crate::commands::workflow::open_in_editor_blocking(
                &require_directory(process)?,
                &editor,
            )
        }
        "pw-stop" => {
            // The menu can be older than the latest scan. Without the scan's
            // entry there is no name to confirm or to check the PID against.
            let Some(process) = process else {
                app.state::<PortPoller>().request_scan();
                return Err(format!(
                    "PID {pid} is no longer listening, so it was not stopped."
                ));
            };
            if !confirm_stop(app, process) {
                return Ok(());
            }
            crate::process_actions::stop_process(
                app,
                pid,
                false,
                crate::process_actions::SeenProcess {
                    name: Some(&process.name),
                    started_at: Some(process.started_at),
                },
            )?;
            // Nothing else rescans after a tray stop, so with manual refresh
            // the stopped process would stay listed indefinitely.
            app.state::<PortPoller>().request_scan();
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Native confirmation before terminating a process — restores the safety the
/// removed popover's StopDialog provided. Runs on a background thread (the
/// caller is `spawn_blocking`), so blocking on the user's response is fine.
fn confirm_stop(app: &AppHandle, process: &PortProcess) -> bool {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

    let name = &process.name;
    let message = if process.is_system_service {
        format!(
            "{name} is a system service. Stopping it may affect your system.\n\nStop it anyway?"
        )
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
        .map(|process| process.project_dir().to_string())
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
    // Scans at once and goes back to the normal pace.
    app.state::<PortPoller>().set_window_visible(true);
}

/// Hides the main window to the tray. The poller slows down while nothing
/// is there to show the result.
pub fn hide_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    app.state::<PortPoller>().set_window_visible(false);
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
        hide_main_window(app);
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
pub fn set_menu_bar_mode(app: AppHandle, enabled: bool) -> Result<(), String> {
    apply_menu_bar_mode(&app, enabled)
}
