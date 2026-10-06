//! The app's settings, kept in one JSON file in the app's config folder and
//! read at launch. The tray and the poller start with what the user chose,
//! instead of running on defaults until the window has loaded and told them.

use std::collections::HashSet;
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
}

pub struct SettingsStore {
    /// None when the app's config folder cannot be worked out; settings then
    /// last for the session.
    file: Option<PathBuf>,
    state: Mutex<State>,
}

struct State {
    settings: Settings,
    /// Goes up with every change, so the window can tell a newer snapshot
    /// from an older one that reaches it late.
    revision: u64,
    /// Whether the window has handed over the settings it kept itself
    /// before this file existed.
    adopted: bool,
    /// What has been changed since launch. If the tray changes a setting
    /// before the window hands its old ones over, the newer choice stands.
    touched: HashSet<String>,
}

/// The settings as of one revision.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Snapshot {
    pub settings: Settings,
    pub revision: u64,
}

pub struct Change {
    pub before: Settings,
    pub after: Snapshot,
}

impl SettingsStore {
    pub fn load(file: Option<PathBuf>) -> Self {
        let (settings, adopted) = file
            .as_deref()
            .and_then(|file| std::fs::read_to_string(file).ok())
            .and_then(|json| read_stored(&json))
            .unwrap_or_default();

        Self {
            file,
            state: Mutex::new(State {
                settings,
                revision: 0,
                adopted,
                touched: HashSet::new(),
            }),
        }
    }

    pub fn get(&self) -> Settings {
        self.lock().settings.clone()
    }

    pub fn snapshot(&self) -> Snapshot {
        let state = self.lock();
        Snapshot {
            settings: state.settings.clone(),
            revision: state.revision,
        }
    }

    pub fn is_adopted(&self) -> bool {
        self.lock().adopted
    }

    /// Replaces the keys of `patch` and saves. Nothing changes when the
    /// patch is refused.
    pub fn update(&self, patch: Map<String, Value>) -> Result<Change, String> {
        let mut state = self.lock();
        let after = state.settings.with(patch.clone())?;
        state.touched.extend(patch.into_iter().map(|(key, _)| key));
        Ok(self.commit(&mut state, after))
    }

    /// Takes over the settings the window kept itself in earlier versions.
    /// Each is used unless it has been changed since launch, or cannot be
    /// read. Done once: later calls change nothing.
    pub fn adopt(&self, legacy: Map<String, Value>) -> Change {
        let mut state = self.lock();
        let mut after = state.settings.clone();
        if !state.adopted {
            for (key, value) in legacy {
                if state.touched.contains(&key) {
                    continue;
                }
                if let Ok(next) = after.with(Map::from_iter([(key, value)])) {
                    after = next;
                }
            }
        }
        state.adopted = true;
        self.commit(&mut state, after)
    }

    fn commit(&self, state: &mut State, after: Settings) -> Change {
        let before = std::mem::replace(&mut state.settings, after);
        state.revision += 1;
        // Kept for the session even when the file cannot be written.
        if let Some(file) = &self.file {
            if let Err(error) = save(file, &state.settings, state.adopted) {
                eprintln!("Failed to save settings: {error}");
            }
        }
        Change {
            before,
            after: Snapshot {
                settings: state.settings.clone(),
                revision: state.revision,
            },
        }
    }

    // A panic while the lock was held leaves the settings as they were
    // before the update that panicked; they are still good to use.
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

const ADOPTED_KEY: &str = "windowSettingsAdopted";
const SETTINGS_KEY: &str = "settings";

// The settings and whether the window's own have been adopted. None for a
// file that is not this app's.
fn read_stored(json: &str) -> Option<(Settings, bool)> {
    let stored: Map<String, Value> = serde_json::from_str(json).ok()?;
    let adopted = stored.get(ADOPTED_KEY) == Some(&Value::Bool(true));

    // Key by key, so that one value this version cannot read does not cost
    // the user the rest of their settings.
    let mut settings = Settings::default();
    if let Some(Value::Object(values)) = stored.get(SETTINGS_KEY) {
        for (key, value) in values {
            if let Ok(next) = settings.with(Map::from_iter([(key.clone(), value.clone())])) {
                settings = next;
            }
        }
    }
    Some((settings, adopted))
}

// Written beside the file and moved over it, so a crash part-way leaves the
// old settings and not half of the new ones.
fn save(file: &std::path::Path, settings: &Settings, adopted: bool) -> std::io::Result<()> {
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder)?;
    }
    let stored = serde_json::json!({ ADOPTED_KEY: adopted, SETTINGS_KEY: settings });
    let json = serde_json::to_vec_pretty(&stored).map_err(std::io::Error::other)?;
    let partial = file.with_extension("json.partial");
    std::fs::write(&partial, json)?;
    std::fs::rename(&partial, file)
}

/// Changes settings on behalf of the window or the tray: saves them, makes
/// the poller and the tray follow, and tells the window what they now are.
pub fn update(app: &AppHandle, patch: Map<String, Value>) -> Result<Snapshot, String> {
    let change = app.state::<SettingsStore>().update(patch)?;
    Ok(follow(app, change))
}

/// Takes over what the window kept in earlier versions; see
/// `SettingsStore::adopt`.
pub fn adopt(app: &AppHandle, legacy: Map<String, Value>) -> Snapshot {
    let change = app.state::<SettingsStore>().adopt(legacy);
    follow(app, change)
}

fn follow(app: &AppHandle, change: Change) -> Snapshot {
    let Change { before, after } = change;

    apply_to_poller(app, &after.settings);
    if before.menu_bar_mode != after.settings.menu_bar_mode {
        if let Err(error) = crate::tray::apply_menu_bar_mode(app, after.settings.menu_bar_mode) {
            eprintln!("Failed to apply menu bar mode: {error}");
        }
    }
    let _ = app.emit("settings-changed", &after);
    after
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

    fn file_in(dir: &tempfile::TempDir) -> PathBuf {
        dir.path().join("config").join("settings.json")
    }

    fn store_in(dir: &tempfile::TempDir) -> SettingsStore {
        SettingsStore::load(Some(file_in(dir)))
    }

    #[test]
    fn a_first_launch_has_defaults_and_nothing_adopted() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);

        assert_eq!(store.get(), Settings::default());
        assert!(!store.is_adopted());
        assert_eq!(store.snapshot().revision, 0);
    }

    #[test]
    fn an_update_is_there_at_the_next_launch() {
        let dir = tempfile::tempdir().unwrap();
        let change = store_in(&dir)
            .update(patch(json!({
                "menuBarMode": true,
                "refreshIntervalMs": 10000,
                "watchedPorts": [3000, 8080],
                "pinnedPaths": ["/Users/dev/app"],
                "changeToastsMutedUntil": null,
            })))
            .unwrap();
        assert_eq!(change.before, Settings::default());
        let after = change.after.settings;

        assert_eq!(store_in(&dir).get(), after);
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
        let after = store
            .update(patch(json!({ "useHttpsForLocalhost": true })))
            .unwrap()
            .after
            .settings;

        assert!(after.include_udp);
        assert!(after.use_https_for_localhost);
        assert_eq!(after.window["searchField"], json!("port"));
    }

    #[test]
    fn every_change_has_a_higher_revision() {
        let store = SettingsStore::load(None);
        let first = store.update(patch(json!({ "includeUdp": true }))).unwrap();
        let second = store.adopt(Map::new());
        let third = store.update(patch(json!({ "includeUdp": false }))).unwrap();

        assert_eq!(
            [
                first.after.revision,
                second.after.revision,
                third.after.revision
            ],
            [1, 2, 3]
        );
        assert_eq!(store.snapshot(), third.after);
    }

    #[test]
    fn a_patch_of_the_wrong_type_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        store.update(patch(json!({ "includeUdp": true }))).unwrap();
        let before = store.snapshot();

        for bad in [
            json!({ "allowSystemProcessActions": "yes" }),
            json!({ "watchedPorts": [3000.5] }),
            json!({ "watchedPorts": [70000] }),
            json!({ "refreshIntervalMs": -1, "menuBarMode": true }),
        ] {
            let error = store.update(patch(bad)).err().unwrap();
            assert!(error.starts_with("Invalid settings"), "{error}");
        }

        assert_eq!(store.snapshot(), before);
        assert_eq!(store_in(&dir).get(), before.settings);
    }

    #[test]
    fn the_file_holds_the_settings_in_camel_case_and_whether_they_were_adopted() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        store
            .update(patch(json!({ "hideSystemServices": true })))
            .unwrap();

        let read = || -> Value {
            serde_json::from_str(&std::fs::read_to_string(file_in(&dir)).unwrap()).unwrap()
        };
        assert_eq!(read()["windowSettingsAdopted"], json!(false));
        assert_eq!(read()["settings"]["refreshIntervalMs"], json!(3000));
        assert_eq!(read()["settings"]["preferredEditor"], json!("cursor"));
        assert_eq!(read()["settings"]["hideSystemServices"], json!(true));
        assert!(read()["settings"].get("window").is_none());

        store.adopt(Map::new());
        assert_eq!(read()["windowSettingsAdopted"], json!(true));
    }

    // The first launch of a version that keeps settings here: the window
    // hands over the ones it had.
    #[test]
    fn the_windows_old_settings_are_adopted_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let adopted = store
            .adopt(patch(json!({
                "includeUdp": true,
                "pinnedPaths": ["/Users/dev/app"],
                "preferredEditor": "code",
            })))
            .after
            .settings;

        assert!(adopted.include_udp);
        assert_eq!(adopted.preferred_editor, "code");
        assert_eq!(adopted.window["pinnedPaths"], json!(["/Users/dev/app"]));

        // Not a second time, in this launch or a later one.
        for store in [store, store_in(&dir)] {
            assert!(store.is_adopted());
            let again = store.adopt(patch(json!({ "includeUdp": false })));
            assert_eq!(again.after.settings, adopted);
        }
    }

    // The tray is usable before the window has loaded. What the user picks
    // there must neither be undone by the hand-over nor cancel it.
    #[test]
    fn a_change_made_before_the_hand_over_stands_and_the_rest_is_still_adopted() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        store.update(patch(json!({ "menuBarMode": true }))).unwrap();
        assert!(!store.is_adopted());

        let adopted = store
            .adopt(patch(json!({
                "menuBarMode": false,
                "watchedPorts": [3000],
                "pinnedPaths": ["/Users/dev/app"],
            })))
            .after
            .settings;

        assert!(adopted.menu_bar_mode);
        assert_eq!(adopted.watched_ports, vec![3000]);
        assert_eq!(adopted.window["pinnedPaths"], json!(["/Users/dev/app"]));
    }

    #[test]
    fn an_old_setting_that_cannot_be_read_is_left_out_of_the_hand_over() {
        let store = SettingsStore::load(None);
        let adopted = store
            .adopt(patch(json!({
                "refreshIntervalMs": "fast",
                "watchedPorts": [80, "x"],
                "includeUdp": true,
            })))
            .after
            .settings;

        assert_eq!(adopted.refresh_interval_ms, 3000);
        assert!(adopted.watched_ports.is_empty());
        assert!(adopted.include_udp);
    }

    // One value this version cannot read must not reset everything else.
    #[test]
    fn a_stored_value_that_cannot_be_read_only_costs_itself() {
        let (settings, adopted) = read_stored(
            r#"{"windowSettingsAdopted":true,"settings":{"refreshIntervalMs":"fast","includeUdp":true,"watchedPorts":[80,"x"],"menuBarMode":true,"groupByDirectory":true}}"#,
        )
        .unwrap();

        assert!(adopted);
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
        for contents in ["", "not json", "[1, 2]", "null", r#"{"settings":7}"#] {
            std::fs::write(&file, contents).unwrap();
            let store = SettingsStore::load(Some(file.clone()));
            assert_eq!(store.get(), Settings::default(), "{contents:?}");
            assert!(!store.is_adopted(), "{contents:?}");
        }
    }

    #[test]
    fn without_a_config_folder_settings_last_for_the_session() {
        let store = SettingsStore::load(None);
        let after = store
            .update(patch(json!({ "includeUdp": true })))
            .unwrap()
            .after
            .settings;

        assert!(after.include_udp);
        assert!(store.get().include_udp);
    }

    #[test]
    fn saving_leaves_only_the_settings_file_behind() {
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
