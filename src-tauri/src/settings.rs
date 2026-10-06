//! The app's settings, kept in one JSON file in the app's config folder and
//! read at launch. The tray and the poller start with what the user chose,
//! instead of running on defaults until the window has loaded and told them.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::poller::PortPoller;

/// The settings the backend acts on are typed. Everything else belongs to
/// the window (filters, pins, toast preferences) and is stored and handed
/// back as it was sent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// 0 turns periodic scans off.
    pub refresh_interval_ms: u64,
    pub include_udp: bool,
    pub allow_system_process_actions: bool,
    pub use_https_for_localhost: bool,
    pub preferred_editor: String,
    pub menu_bar_mode: bool,
    pub watched_ports: Vec<u16>,
    pub watched_port_notifications: bool,
    #[serde(flatten)]
    pub window: Map<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            refresh_interval_ms: 3000,
            include_udp: false,
            allow_system_process_actions: false,
            use_https_for_localhost: false,
            preferred_editor: "cursor".to_string(),
            menu_bar_mode: false,
            watched_ports: Vec::new(),
            watched_port_notifications: false,
            window: Map::new(),
        }
    }
}

impl Settings {
    /// With the window hidden the poller slows down, unless a watched port
    /// is waiting to raise a desktop alert.
    pub fn watch_while_hidden(&self) -> bool {
        self.watched_port_notifications && !self.watched_ports.is_empty()
    }

    /// These settings with the keys of `patch` replaced. A value of the
    /// wrong type for a setting the backend acts on is an error.
    fn with(&self, patch: Map<String, Value>) -> Result<Settings, String> {
        let mut merged = match serde_json::to_value(self) {
            Ok(Value::Object(merged)) => merged,
            _ => return Err("Settings could not be read".into()),
        };
        merged.extend(patch);
        serde_json::from_value(Value::Object(merged)).map_err(|e| format!("Invalid settings: {e}"))
    }

    // Key by key, so that one value this version cannot read does not cost
    // the user the rest of their settings.
    fn from_stored(json: &str) -> Option<Settings> {
        let stored: Map<String, Value> = serde_json::from_str(json).ok()?;
        let mut settings = Settings::default();
        for (key, value) in stored {
            if let Ok(next) = settings.with(Map::from_iter([(key, value)])) {
                settings = next;
            }
        }
        Some(settings)
    }
}

pub struct SettingsStore {
    /// None when the app's config folder cannot be worked out; settings then
    /// last for the session.
    file: Option<PathBuf>,
    state: Mutex<State>,
}

struct State {
    settings: Settings,
    /// Whether settings have ever been saved. Until they have, the window
    /// may still hold the ones it kept itself in earlier versions.
    stored: bool,
}

impl SettingsStore {
    pub fn load(file: Option<PathBuf>) -> Self {
        let stored = file
            .as_deref()
            .and_then(|file| std::fs::read_to_string(file).ok())
            .and_then(|json| Settings::from_stored(&json));

        Self {
            file,
            state: Mutex::new(State {
                stored: stored.is_some(),
                settings: stored.unwrap_or_default(),
            }),
        }
    }

    pub fn get(&self) -> Settings {
        self.lock().settings.clone()
    }

    pub fn is_stored(&self) -> bool {
        self.lock().stored
    }

    /// Replaces the keys of `patch` and saves. Returns the settings before
    /// and after. Nothing changes when the patch is refused.
    pub fn update(&self, patch: Map<String, Value>) -> Result<(Settings, Settings), String> {
        let mut state = self.lock();
        let before = state.settings.clone();
        let after = before.with(patch)?;

        // Kept for the session even when the file cannot be written.
        state.settings = after.clone();
        state.stored = true;
        if let Some(file) = &self.file {
            if let Err(error) = save(file, &after) {
                eprintln!("Failed to save settings: {error}");
            }
        }
        Ok((before, after))
    }

    // A panic while the lock was held leaves the settings as they were
    // before the update that panicked; they are still good to use.
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

// Written beside the file and moved over it, so a crash part-way leaves the
// old settings and not half of the new ones.
fn save(file: &std::path::Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let json = serde_json::to_vec_pretty(settings).map_err(std::io::Error::other)?;
    let partial = file.with_extension("json.partial");
    std::fs::write(&partial, json)?;
    std::fs::rename(&partial, file)
}

/// Changes settings on behalf of the window or the tray: saves them, makes
/// the poller and the tray follow, and tells the window what they now are.
pub fn update(app: &AppHandle, patch: Map<String, Value>) -> Result<Settings, String> {
    let (before, after) = app.state::<SettingsStore>().update(patch)?;

    apply_to_poller(app, &after);
    if before.menu_bar_mode != after.menu_bar_mode {
        if let Err(error) = crate::tray::apply_menu_bar_mode(app, after.menu_bar_mode) {
            eprintln!("Failed to apply menu bar mode: {error}");
        }
    }
    let _ = app.emit("settings-changed", &after);
    Ok(after)
}

pub fn apply_to_poller(app: &AppHandle, settings: &Settings) {
    app.state::<PortPoller>().set_scan_settings(
        settings.refresh_interval_ms,
        settings.include_udp,
        settings.watch_while_hidden(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn patch(value: Value) -> Map<String, Value> {
        match value {
            Value::Object(map) => map,
            other => panic!("not an object: {other}"),
        }
    }

    fn store_in(dir: &tempfile::TempDir) -> SettingsStore {
        SettingsStore::load(Some(dir.path().join("config").join("settings.json")))
    }

    #[test]
    fn a_first_launch_has_defaults_and_nothing_stored() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);

        assert_eq!(store.get(), Settings::default());
        assert!(!store.is_stored());
    }

    #[test]
    fn an_update_is_there_at_the_next_launch() {
        let dir = tempfile::tempdir().unwrap();
        let (before, after) = store_in(&dir)
            .update(patch(json!({
                "menuBarMode": true,
                "refreshIntervalMs": 10000,
                "watchedPorts": [3000, 8080],
                "pinnedPaths": ["/Users/dev/app"],
                "changeToastsMutedUntil": null,
            })))
            .unwrap();
        assert_eq!(before, Settings::default());

        let relaunched = store_in(&dir);
        assert!(relaunched.is_stored());
        assert_eq!(relaunched.get(), after);
        assert!(after.menu_bar_mode);
        assert_eq!(after.refresh_interval_ms, 10_000);
        assert_eq!(after.watched_ports, vec![3000, 8080]);
        // What only the window uses comes back as it was sent.
        assert_eq!(after.window["pinnedPaths"], json!(["/Users/dev/app"]));
        assert_eq!(after.window["changeToastsMutedUntil"], Value::Null);
    }

    #[test]
    fn a_patch_leaves_the_other_settings_alone() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        store
            .update(patch(json!({ "includeUdp": true, "searchField": "port" })))
            .unwrap();
        let (_, after) = store
            .update(patch(json!({ "useHttpsForLocalhost": true })))
            .unwrap();

        assert!(after.include_udp);
        assert!(after.use_https_for_localhost);
        assert_eq!(after.window["searchField"], json!("port"));
    }

    #[test]
    fn a_patch_of_the_wrong_type_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        store.update(patch(json!({ "includeUdp": true }))).unwrap();

        for bad in [
            json!({ "allowSystemProcessActions": "yes" }),
            json!({ "watchedPorts": [3000.5] }),
            json!({ "watchedPorts": [70000] }),
            json!({ "refreshIntervalMs": -1, "menuBarMode": true }),
        ] {
            let error = store.update(patch(bad)).unwrap_err();
            assert!(error.starts_with("Invalid settings"), "{error}");
        }

        let settings = store.get();
        assert!(settings.include_udp);
        assert!(!settings.menu_bar_mode);
        assert_eq!(store_in(&dir).get(), settings);
    }

    #[test]
    fn stored_json_is_camel_case_and_flat() {
        let json = serde_json::to_value(Settings {
            window: patch(json!({ "hideSystemServices": true })),
            ..Settings::default()
        })
        .unwrap();

        assert_eq!(json["refreshIntervalMs"], json!(3000));
        assert_eq!(json["preferredEditor"], json!("cursor"));
        assert_eq!(json["hideSystemServices"], json!(true));
        assert!(json.get("window").is_none());
    }

    // One value this version cannot read must not reset everything else.
    #[test]
    fn a_stored_value_that_cannot_be_read_only_costs_itself() {
        let settings = Settings::from_stored(
            r#"{"refreshIntervalMs":"fast","includeUdp":true,"watchedPorts":[80,"x"],"menuBarMode":true,"groupByDirectory":true}"#,
        )
        .unwrap();

        assert_eq!(settings.refresh_interval_ms, 3000);
        assert!(settings.watched_ports.is_empty());
        assert!(settings.include_udp);
        assert!(settings.menu_bar_mode);
        assert_eq!(settings.window["groupByDirectory"], json!(true));
    }

    #[test]
    fn a_file_that_is_not_settings_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        for contents in ["", "not json", "[1, 2]", "null"] {
            std::fs::write(&file, contents).unwrap();
            let store = SettingsStore::load(Some(file.clone()));
            assert_eq!(store.get(), Settings::default(), "{contents:?}");
            assert!(!store.is_stored(), "{contents:?}");
        }
    }

    #[test]
    fn without_a_config_folder_settings_last_for_the_session() {
        let store = SettingsStore::load(None);
        let (_, after) = store.update(patch(json!({ "includeUdp": true }))).unwrap();

        assert!(after.include_udp);
        assert!(store.get().include_udp);
        assert!(store.is_stored());
    }

    #[test]
    fn a_failed_save_leaves_no_partial_file_in_place_of_the_settings() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        store.update(patch(json!({ "includeUdp": true }))).unwrap();

        let folder = dir.path().join("config");
        let names: Vec<String> = std::fs::read_dir(&folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["settings.json"]);
    }

    #[test]
    fn a_watched_port_keeps_the_poller_at_full_pace_only_with_alerts_on() {
        let with = |ports: &[u16], alerts| Settings {
            watched_ports: ports.to_vec(),
            watched_port_notifications: alerts,
            ..Settings::default()
        };
        assert!(with(&[3000], true).watch_while_hidden());
        assert!(!with(&[3000], false).watch_while_hidden());
        assert!(!with(&[], true).watch_while_hidden());
    }
}
