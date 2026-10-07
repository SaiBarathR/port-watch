use serde::Serialize;
use serde_json::{Map, Value};
use tauri::{AppHandle, Manager};

use crate::settings::{SettingsStore, Snapshot};

#[derive(Serialize)]
pub struct SettingsReply {
    #[serde(flatten)]
    snapshot: Snapshot,
    /// False until the window has handed over the settings it kept itself
    /// in earlier versions, which it does with `adopt_window_settings`.
    adopted: bool,
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> SettingsReply {
    let store = app.state::<SettingsStore>();
    SettingsReply {
        snapshot: store.snapshot(),
        adopted: store.is_adopted(),
    }
}

/// Replaces the settings named in `patch`.
#[tauri::command]
pub fn update_settings(app: AppHandle, patch: Map<String, Value>) -> Result<Snapshot, String> {
    crate::settings::update(&app, patch)
}

#[derive(Serialize)]
pub struct AdoptReply {
    #[serde(flatten)]
    snapshot: Snapshot,
    /// Whether the settings were written to disk. If not, the window keeps
    /// offering its own copy on later launches.
    saved: bool,
}

/// Hands over the settings the window kept in earlier versions. Used once,
/// on the first launch of a version that keeps them in the backend.
#[tauri::command]
pub fn adopt_window_settings(app: AppHandle, legacy: Map<String, Value>) -> AdoptReply {
    let (snapshot, saved) = crate::settings::adopt(&app, legacy);
    AdoptReply { snapshot, saved }
}
