use std::collections::VecDeque;
use std::sync::Mutex;

use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Manager, Wry,
};

use crate::poller::PortPoller;
use crate::scanner::PortProcess;
use crate::settings::SettingsStore;

#[derive(Default)]
pub struct TrayState {
    /// The menus that have been applied, newest last, each with its number.
    /// More than one, because a menu that is open stays on screen after a
    /// scan has replaced it, and a click on it must still find what it
    /// showed.
    menus: VecDeque<(u64, Shown)>,
    /// The last number given to a menu. A number is never used twice, even
    /// for a menu that failed to build.
    numbered: u64,
}

/// How many applied menus are remembered. One can be open while the next is
/// applied; the third is slack.
const MENUS_KEPT: usize = 3;

/// Everything a tray menu shows. A click acts on the process its item was
/// built from, not on whatever holds that PID by the time of the click.
#[derive(Clone, PartialEq)]
struct Shown {
    /// The user's listeners, in menu order.
    processes: Vec<PortProcess>,
    menu_bar_mode: bool,
    editor: String,
}

impl Shown {
    fn now(app: &AppHandle) -> Self {
        let settings = app.state::<SettingsStore>().get();
        let mut processes: Vec<PortProcess> = app
            .state::<PortPoller>()
            .processes()
            .iter()
            .filter(|process| !process.is_system_service)
            .cloned()
            .collect();
        processes.sort_by_key(|process| primary_port(process).unwrap_or(u16::MAX));

        Self {
            processes,
            menu_bar_mode: settings.menu_bar_mode,
            editor: settings.preferred_editor,
        }
    }
}

pub fn setup_tray(app: &AppHandle, menu_bar_mode: bool) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(Mutex::new(TrayState::default()));

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("Missing application icon for tray")?;

    // Before the first scan there is nothing to list; the poller rebuilds
    // the menu as soon as there is.
    let menu = build_menu(
        app,
        0,
        &Shown {
            processes: Vec::new(),
            menu_bar_mode,
            editor: String::new(),
        },
    )?;

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
// What a menu item does
// ---------------------------------------------------------------------------

/// What a menu item does. An item's id is one of these, written out, so a
/// click is read back into the same type that built the item.
#[derive(Debug, Clone, PartialEq, Eq)]
enum TrayAction {
    OpenWindow,
    Refresh,
    ToggleMenuBarMode,
    Quit,
    /// Something done to one listed process: the number of the menu the
    /// item is in, and the process's row id in that menu.
    Port(PortAction, u64, String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortAction {
    Open,
    CopyUrl,
    Reveal,
    Terminal,
    Editor,
    Stop,
}

impl PortAction {
    const ALL: [PortAction; 6] = [
        PortAction::Open,
        PortAction::CopyUrl,
        PortAction::Reveal,
        PortAction::Terminal,
        PortAction::Editor,
        PortAction::Stop,
    ];

    fn name(self) -> &'static str {
        match self {
            PortAction::Open => "open",
            PortAction::CopyUrl => "copy-url",
            PortAction::Reveal => "reveal",
            PortAction::Terminal => "terminal",
            PortAction::Editor => "editor",
            PortAction::Stop => "stop",
        }
    }
}

impl TrayAction {
    fn id(&self) -> String {
        match self {
            TrayAction::OpenWindow => "window".into(),
            TrayAction::Refresh => "refresh".into(),
            TrayAction::ToggleMenuBarMode => "menu-bar-mode".into(),
            TrayAction::Quit => "quit".into(),
            TrayAction::Port(action, menu, row) => {
                format!("port:{menu}:{}:{row}", action.name())
            }
        }
    }

    /// None for an id that is not an action's: the menu's heading, or
    /// anything this version did not write.
    fn parse(id: &str) -> Option<Self> {
        match id {
            "window" => return Some(TrayAction::OpenWindow),
            "refresh" => return Some(TrayAction::Refresh),
            "menu-bar-mode" => return Some(TrayAction::ToggleMenuBarMode),
            "quit" => return Some(TrayAction::Quit),
            _ => {}
        }

        // A row id can hold colons itself ("socket-tcp-[::1]-3000"), so it
        // is whatever follows the third one.
        let mut parts = id.splitn(4, ':');
        if parts.next()? != "port" {
            return None;
        }
        let menu = parts.next()?.parse().ok()?;
        let name = parts.next()?;
        let action = PortAction::ALL
            .into_iter()
            .find(|action| action.name() == name)?;
        let row = parts.next().filter(|row| !row.is_empty())?;
        Some(TrayAction::Port(action, menu, row.to_string()))
    }
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

#[cfg(target_os = "macos")]
const REVEAL_LABEL: &str = "Show in Finder";
#[cfg(target_os = "windows")]
const REVEAL_LABEL: &str = "Show in Explorer";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const REVEAL_LABEL: &str = "Show in File Manager";

fn editor_label(editor: &str) -> &'static str {
    match editor {
        "code" => "Open in VS Code",
        "cursor" => "Open in Cursor",
        _ => "Open in Editor",
    }
}

fn build_port_submenu(
    app: &AppHandle,
    menu: u64,
    process: &PortProcess,
    editor: &str,
) -> tauri::Result<Submenu<Wry>> {
    let port = primary_port(process);
    let has_dir = !process.project_dir().is_empty();
    let item = |action: PortAction, label: &str, enabled: bool| {
        MenuItem::with_id(
            app,
            TrayAction::Port(action, menu, process.id.clone()).id(),
            label,
            enabled,
            None::<&str>,
        )
    };

    let title = match port {
        Some(port) => format!("{port}  ·  {}", process.name),
        None => process.name.clone(),
    };
    let open = item(
        PortAction::Open,
        &match port {
            Some(port) => format!("Open localhost:{port}"),
            None => "Open in Browser".to_string(),
        },
        port.is_some(),
    )?;
    let copy = item(PortAction::CopyUrl, "Copy URL", port.is_some())?;
    let reveal = item(PortAction::Reveal, REVEAL_LABEL, has_dir)?;
    let terminal = item(PortAction::Terminal, "Open in Terminal", has_dir)?;
    let editor = item(PortAction::Editor, editor_label(editor), has_dir)?;
    // PID 0 is a listener whose owner the scan could not see: there is no
    // process to stop.
    let stop = item(PortAction::Stop, "Stop…", process.pid != 0)?;
    let sep_open = PredefinedMenuItem::separator(app)?;
    let sep_stop = PredefinedMenuItem::separator(app)?;

    Submenu::with_items(
        app,
        title,
        true,
        &[
            &open, &copy, &sep_open, &reveal, &terminal, &editor, &sep_stop, &stop,
        ],
    )
}

// `number` goes into the id of every item that acts on a process, and is
// how a click finds the processes this menu was built from.
fn build_menu(app: &AppHandle, number: u64, shown: &Shown) -> tauri::Result<Menu<Wry>> {
    let count = shown.processes.len();
    let header_label = if count == 1 {
        "Port Watch — 1 listener".to_string()
    } else {
        format!("Port Watch — {count} listeners")
    };
    let header = MenuItem::with_id(app, "heading", header_label, false, None::<&str>)?;

    let menu = Menu::new(app)?;
    menu.append(&header)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    if shown.processes.is_empty() {
        let empty = MenuItem::with_id(
            app,
            "nothing-listening",
            "No dev servers listening",
            false,
            None::<&str>,
        )?;
        menu.append(&empty)?;
    } else {
        for process in &shown.processes {
            menu.append(&build_port_submenu(app, number, process, &shown.editor)?)?;
        }
    }

    menu.append(&PredefinedMenuItem::separator(app)?)?;

    let open_window = MenuItem::with_id(
        app,
        TrayAction::OpenWindow.id(),
        "Open Full Window",
        true,
        None::<&str>,
    )?;
    let refresh = MenuItem::with_id(app, TrayAction::Refresh.id(), "Refresh", true, None::<&str>)?;
    let menu_bar_mode = CheckMenuItem::with_id(
        app,
        TrayAction::ToggleMenuBarMode.id(),
        "Menu bar mode",
        true,
        shown.menu_bar_mode,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, TrayAction::Quit.id(), "Quit", true, None::<&str>)?;

    menu.append(&open_window)?;
    menu.append(&refresh)?;
    menu.append(&menu_bar_mode)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&quit)?;

    Ok(menu)
}

/// Rebuilds the native tray menu from the poller's latest scan and the
/// settings. Cheap to call after every scan: it only touches the menu when
/// something the menu shows has changed. The menu must be changed on the
/// main thread.
pub fn rebuild_tray_menu(app: &AppHandle) {
    let shown = Shown::now(app);
    let Some(state) = app.try_state::<Mutex<TrayState>>() else {
        return;
    };
    let number = {
        let Ok(mut state) = state.lock() else {
            return;
        };
        if state
            .menus
            .back()
            .is_some_and(|(_, latest)| *latest == shown)
        {
            return;
        }
        state.numbered += 1;
        state.numbered
    };

    let app_main = app.clone();
    let _ = app.run_on_main_thread(move || match build_menu(&app_main, number, &shown) {
        Ok(menu) => {
            let Some(tray) = app_main.tray_by_id("main") else {
                return;
            };
            let _ = tray.set_menu(Some(menu));
            let count = shown.processes.len();
            let label = if count == 1 {
                "1 listener".to_string()
            } else {
                format!("{count} listeners")
            };
            let _ = tray.set_tooltip(Some(&label));

            // Recorded only once the menu is in place: a build that failed
            // must be tried again after the next scan, not taken as done.
            if let Some(state) = app_main.try_state::<Mutex<TrayState>>() {
                if let Ok(mut state) = state.lock() {
                    remember(&mut state.menus, number, shown);
                }
            }
        }
        Err(error) => eprintln!("Failed to build tray menu: {error}"),
    });
}

fn remember(menus: &mut VecDeque<(u64, Shown)>, number: u64, shown: Shown) {
    menus.push_back((number, shown));
    while menus.len() > MENUS_KEPT {
        menus.pop_front();
    }
}

/// The process a menu item was built from, and the editor that menu named
/// in its "Open in …" items. None for an item of a menu too old to be
/// remembered, which then does nothing.
fn shown_process(
    menus: &VecDeque<(u64, Shown)>,
    number: u64,
    row: &str,
) -> Option<(PortProcess, String)> {
    let (_, shown) = menus.iter().find(|(kept, _)| *kept == number)?;
    let process = shown.processes.iter().find(|process| process.id == row)?;
    Some((process.clone(), shown.editor.clone()))
}

// ---------------------------------------------------------------------------
// Menu events
// ---------------------------------------------------------------------------

fn handle_menu_event(app: &AppHandle, id: &str) {
    match TrayAction::parse(id) {
        Some(TrayAction::OpenWindow) => show_main_window(app),
        Some(TrayAction::Refresh) => app.state::<PortPoller>().request_scan(),
        Some(TrayAction::ToggleMenuBarMode) => {
            let enabled = !app.state::<SettingsStore>().get().menu_bar_mode;
            let patch = serde_json::Map::from_iter([("menuBarMode".to_string(), enabled.into())]);
            if let Err(error) = crate::settings::update(app, patch) {
                eprintln!("Failed to change menu bar mode: {error}");
            }
        }
        Some(TrayAction::Quit) => app.exit(0),
        Some(TrayAction::Port(action, menu, row)) => {
            // The process the item was built from. The menu on screen can be
            // older than the latest scan, or than the latest menu, and the
            // item said what it would act on.
            let clicked = app.try_state::<Mutex<TrayState>>().and_then(|state| {
                let state = state.lock().ok()?;
                shown_process(&state.menus, menu, &row)
            });
            let Some((process, editor)) = clicked else {
                app.state::<PortPoller>().request_scan();
                return;
            };
            let app = app.clone();
            // Off the main thread: a stop can wait for seconds, and the
            // launchers wait on a child process.
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(error) = run_port_action(&app, action, &process, &editor) {
                    notify_error(&app, &error);
                }
            });
        }
        None => {}
    }
}

// `editor` is the one the clicked menu named, which the setting may have
// moved on from while that menu was open.
fn run_port_action(
    app: &AppHandle,
    action: PortAction,
    process: &PortProcess,
    editor: &str,
) -> Result<(), String> {
    let url = || {
        let use_https = app.state::<SettingsStore>().get().use_https_for_localhost;
        primary_port(process)
            .map(|port| localhost_url(port, use_https))
            .ok_or("No port available")
    };

    match action {
        PortAction::Open => crate::commands::workflow::open_url(app.clone(), url()?),
        PortAction::CopyUrl => crate::platform::shell::copy_to_clipboard(&url()?),
        PortAction::Reveal => {
            crate::commands::filesystem::open_in_finder_blocking(&require_directory(process)?)
        }
        PortAction::Terminal => {
            crate::commands::workflow::open_in_terminal_blocking(&require_directory(process)?)
        }
        PortAction::Editor => {
            crate::commands::workflow::open_in_editor_blocking(&require_directory(process)?, editor)
        }
        PortAction::Stop => {
            if !confirm_stop(app, process) {
                return Ok(());
            }
            // Checked against the latest scan by name and start time: if the
            // PID has gone to another process since the menu was built, the
            // stop is refused.
            let stopped = crate::process_actions::stop_process(
                app,
                process.pid,
                false,
                crate::process_actions::SeenProcess {
                    name: Some(&process.name),
                    started_at: Some(process.started_at),
                },
            );
            // Nothing else rescans after a tray stop, so with auto-refresh
            // off a stopped process would stay listed, and a refused stop
            // would leave the stale menu in place.
            app.state::<PortPoller>().request_scan();
            stopped
        }
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

fn require_directory(process: &PortProcess) -> Result<String, String> {
    Some(process.project_dir().to_string())
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

/// What the stored setting means at launch. The window is created hidden:
/// in menu bar mode it stays hidden and the Dock icon never appears, and
/// otherwise it is shown.
pub fn apply_launch_mode(app: &AppHandle, menu_bar_mode: bool) {
    if !menu_bar_mode {
        show_main_window(app);
        return;
    }
    if let Err(error) = set_dock_icon_hidden(app, true) {
        eprintln!("{error}");
    }
    app.state::<PortPoller>().set_window_visible(false);
}

/// Follows a change of the setting, which has already been saved.
pub fn apply_menu_bar_mode(app: &AppHandle, enabled: bool) -> Result<(), String> {
    set_dock_icon_hidden(app, enabled)?;

    if enabled {
        hide_main_window(app);
    } else {
        show_main_window(app);
    }
    Ok(())
}

#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
fn set_dock_icon_hidden(app: &AppHandle, hidden: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let policy = if hidden {
            tauri::ActivationPolicy::Accessory
        } else {
            tauri::ActivationPolicy::Regular
        };
        app.set_activation_policy(policy)
            .map_err(|e| format!("Failed to set activation policy: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_reads_back_from_its_id() {
        let mut actions = vec![
            TrayAction::OpenWindow,
            TrayAction::Refresh,
            TrayAction::ToggleMenuBarMode,
            TrayAction::Quit,
        ];
        for action in PortAction::ALL {
            actions.push(TrayAction::Port(action, 1, "pid-4242".into()));
            // A listener with no visible owner is named by its socket, and
            // an IPv6 address brings colons of its own.
            actions.push(TrayAction::Port(
                action,
                18_446_744_073_709_551_615,
                "socket-tcp-[::1]-3000#2".into(),
            ));
        }

        for action in actions {
            assert_eq!(TrayAction::parse(&action.id()), Some(action));
        }
    }

    #[test]
    fn ids_that_are_not_actions_do_nothing() {
        for id in [
            "",
            "heading",
            "nothing-listening",
            "port",
            "port:1",
            "port:1:stop",
            "port:1:stop:",
            "port:1:explode:pid-1",
            // No menu number.
            "port:stop:pid-1",
            // What earlier versions wrote; a menu is never that old, but the
            // ids must not be read as something else.
            "pw-stop:4242",
            "tray-quit",
        ] {
            assert_eq!(TrayAction::parse(id), None, "{id:?}");
        }
    }

    fn listener(pid: u32, port: u16) -> PortProcess {
        PortProcess {
            id: format!("pid-{pid}"),
            pid,
            name: "node".into(),
            user: "dev".into(),
            ports: vec![crate::scanner::PortBinding {
                address: "*".into(),
                port,
                protocol: "TCP".into(),
            }],
            executable_path: String::new(),
            script_path: None,
            command_line: String::new(),
            working_directory: String::new(),
            project_root: String::new(),
            system_kind: crate::classifier::SystemKind::User,
            is_system_service: false,
            started_at: 1_790_000_000,
            delete_blocked: None,
        }
    }

    fn showing(processes: Vec<PortProcess>) -> Shown {
        showing_with_editor(processes, "cursor")
    }

    fn showing_with_editor(processes: Vec<PortProcess>, editor: &str) -> Shown {
        Shown {
            processes,
            menu_bar_mode: false,
            editor: editor.into(),
        }
    }

    // A menu stays open on screen after a scan has replaced it. The same
    // PID is then in both, listening on a different port in each, and a
    // click on "Open localhost:3000" must not open 4000.
    #[test]
    fn a_click_finds_the_process_as_its_own_menu_showed_it() {
        let mut menus = VecDeque::new();
        remember(&mut menus, 1, showing(vec![listener(42, 3000)]));
        remember(&mut menus, 2, showing(vec![listener(42, 4000)]));

        let port = |number| {
            shown_process(&menus, number, "pid-42").map(|(process, _)| process.ports[0].port)
        };
        assert_eq!(port(1), Some(3000));
        assert_eq!(port(2), Some(4000));
        assert_eq!(shown_process(&menus, 2, "pid-7"), None);
    }

    // The same for the setting an item was labelled from: "Open in Cursor"
    // opens Cursor, even if the editor was changed while that menu was open.
    #[test]
    fn a_click_gets_the_editor_its_own_menu_named() {
        let mut menus = VecDeque::new();
        remember(
            &mut menus,
            1,
            showing_with_editor(vec![listener(42, 3000)], "cursor"),
        );
        remember(
            &mut menus,
            2,
            showing_with_editor(vec![listener(42, 3000)], "code"),
        );

        let editor = |number| shown_process(&menus, number, "pid-42").map(|(_, editor)| editor);
        assert_eq!(editor(1).as_deref(), Some("cursor"));
        assert_eq!(editor(2).as_deref(), Some("code"));
    }

    #[test]
    fn a_menu_too_old_to_be_remembered_does_nothing() {
        let mut menus = VecDeque::new();
        for number in 1..=5 {
            remember(&mut menus, number, showing(vec![listener(42, 3000)]));
        }

        assert_eq!(menus.len(), MENUS_KEPT);
        assert_eq!(shown_process(&menus, 1, "pid-42"), None);
        assert!(shown_process(&menus, 5, "pid-42").is_some());
    }

    #[test]
    fn the_editor_item_names_the_editor() {
        assert_eq!(editor_label("cursor"), "Open in Cursor");
        assert_eq!(editor_label("code"), "Open in VS Code");
        assert_eq!(editor_label(""), "Open in Editor");
    }
}
