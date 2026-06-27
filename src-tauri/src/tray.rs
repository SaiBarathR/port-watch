use std::sync::Mutex;

use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime, WindowEvent,
};
use tauri_plugin_positioner::{Position, WindowExt};

pub struct TrayState {
    pub user_listener_count: u32,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            user_listener_count: 0,
        }
    }
}

pub struct TrayMenuState<R: Runtime> {
    pub menu_bar_mode: CheckMenuItem<R>,
}

pub fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(Mutex::new(TrayState::default()));

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("Missing application icon for tray")?;

    let open_window = MenuItem::with_id(
        app,
        "tray-open-window",
        "Open Full Window",
        true,
        None::<&str>,
    )?;
    let menu_bar_mode = CheckMenuItem::with_id(
        app,
        "tray-menu-bar-mode",
        "Menu bar mode",
        true,
        false,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "tray-quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_window, &menu_bar_mode, &quit])?;

    app.manage(Mutex::new(TrayMenuState {
        menu_bar_mode: menu_bar_mode.clone(),
    }));

    setup_popover_dismiss(app);

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("Port Watch")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "tray-open-window" => show_full_window(app),
            "tray-menu-bar-mode" => {
                let enabled = is_menu_bar_mode_enabled(app);

                if let Err(error) = apply_menu_bar_mode(app, enabled) {
                    eprintln!("Failed to apply menu bar mode: {error}");
                    let _ = sync_menu_bar_mode_check(app, !enabled);
                    return;
                }

                let _ = app.emit("tray-menu-bar-mode-changed", enabled);
            }
            "tray-quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(move |tray, event| {
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);

            if let TrayIconEvent::Click {
                button,
                button_state,
                ..
            } = event
            {
                if button == MouseButton::Left && button_state == MouseButtonState::Up {
                    let app = tray.app_handle();
                    if is_menu_bar_mode_enabled(&app) {
                        toggle_popover(&app);
                    } else {
                        show_main_window(&app);
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn setup_popover_dismiss<R: Runtime>(app: &AppHandle<R>) {
    let app_handle = app.clone();
    if let Some(popover) = app.get_webview_window("popover") {
        popover.on_window_event(move |event| {
            if let WindowEvent::Focused(false) = event {
                hide_popover(&app_handle);
            }
        });
    }
}

fn is_menu_bar_mode_enabled<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<Mutex<TrayMenuState<R>>>()
        .and_then(|state| {
            state
                .lock()
                .ok()
                .and_then(|guard| guard.menu_bar_mode.is_checked().ok())
        })
        .unwrap_or(false)
}

pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn show_full_window<R: Runtime>(app: &AppHandle<R>) {
    show_main_window(app);
}

pub fn show_popover<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("popover") {
        let _ = window.move_window(Position::TrayBottomCenter);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn hide_popover<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("popover") {
        let _ = window.hide();
    }
}

fn toggle_popover<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("popover") {
        if window.is_visible().unwrap_or(false) {
            hide_popover(app);
        } else {
            show_popover(app);
        }
    }
}

fn sync_menu_bar_mode_check<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> Result<(), String> {
    if let Some(state) = app.try_state::<Mutex<TrayMenuState<R>>>() {
        let state = state
            .lock()
            .map_err(|_| "Tray menu state lock poisoned".to_string())?;
        state
            .menu_bar_mode
            .set_checked(enabled)
            .map_err(|e| format!("Failed to sync menu bar mode check: {e}"))?;
    }
    Ok(())
}

fn apply_menu_bar_mode<R: Runtime>(app: &AppHandle<R>, enabled: bool) -> Result<(), String> {
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

    hide_popover(app);

    if enabled {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
    } else {
        show_main_window(app);
    }

    sync_menu_bar_mode_check(app, enabled)
}

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
pub fn set_menu_bar_mode<R: Runtime>(app: AppHandle<R>, enabled: bool) -> Result<(), String> {
    apply_menu_bar_mode(&app, enabled)
}

#[tauri::command]
pub fn show_full_window_command(app: AppHandle) -> Result<(), String> {
    show_full_window(&app);
    Ok(())
}
