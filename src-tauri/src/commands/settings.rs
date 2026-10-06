use serde::Serialize;
use serde_json::{Map, Value};
use tauri::{AppHandle, Manager};

use crate::settings::{Settings, SettingsStore};

#[derive(Serialize)]
pub struct SettingsReply {
    settings: Settings,
    /// False until settings have been saved once: the window may then still
    /// hold the ones it kept itself in earlier versions, and hands them over.
    stored: bool,
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> SettingsReply {
    let store = app.state::<SettingsStore>();
    SettingsReply {
        settings: store.get(),
        stored: store.is_stored(),
    }
}

/// Replaces the settings named in `patch` and returns all of them.
#[tauri::command]
pub fn update_settings(app: AppHandle, patch: Map<String, Value>) -> Result<Settings, String> {
    crate::settings::update(&app, patch)
}
