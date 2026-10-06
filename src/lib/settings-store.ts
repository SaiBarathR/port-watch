import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import {
  DEFAULT_SETTINGS,
  SEARCH_FIELD_OPTIONS,
  type AppSettings,
} from "@/lib/types";

// Settings live in the backend, which reads them at launch: the tray and the
// poller need them before this window has loaded. This is the window's copy.

/** Where the window kept settings itself, before the backend did. */
const LEGACY_KEY = "port-watch-settings";

let current: AppSettings = DEFAULT_SETTINGS;
let connected = false;
// Updates sent and not yet answered. A reply or an event describes the
// settings as of one update, and applying it while a later one is on its way
// would briefly undo that later one on screen.
let unanswered = 0;
const listeners = new Set<() => void>();

function publish(next: AppSettings) {
  current = next;
  for (const listener of listeners) {
    listener();
  }
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function getSettings(): AppSettings {
  return current;
}

export function useSettings(): AppSettings {
  return useSyncExternalStore(subscribe, getSettings);
}

const isBoolean = (value: unknown): value is boolean =>
  typeof value === "boolean";

function isPort(value: unknown): value is number {
  return (
    typeof value === "number" &&
    Number.isInteger(value) &&
    value >= 1 &&
    value <= 65535
  );
}

/**
 * Settings from anything that claims to hold them: the backend's reply, or
 * what an earlier version left in localStorage. A value of the wrong type
 * gives way to the default, so one bad entry does not cost the rest.
 */
export function readSettings(raw: unknown): AppSettings {
  const stored =
    raw && typeof raw === "object" ? (raw as Record<string, unknown>) : {};
  const pick = <K extends keyof AppSettings>(
    key: K,
    isValid: (value: unknown) => value is AppSettings[K],
  ): AppSettings[K] =>
    isValid(stored[key]) ? stored[key] : DEFAULT_SETTINGS[key];

  const settings: AppSettings = {
    hideSystemServices: pick("hideSystemServices", isBoolean),
    hideUserServices: pick("hideUserServices", isBoolean),
    allowSystemProcessActions: pick("allowSystemProcessActions", isBoolean),
    refreshIntervalMs: pick(
      "refreshIntervalMs",
      (value): value is AppSettings["refreshIntervalMs"] =>
        value === 3000 || value === 10000 || value === 0,
    ),
    preferredEditor: pick(
      "preferredEditor",
      (value): value is AppSettings["preferredEditor"] =>
        value === "cursor" || value === "code",
    ),
    groupByDirectory: pick("groupByDirectory", isBoolean),
    showChangeToasts: pick("showChangeToasts", isBoolean),
    changeToastsMutedUntil: pick(
      "changeToastsMutedUntil",
      (value): value is number | null =>
        value === null || (typeof value === "number" && Number.isFinite(value)),
    ),
    menuBarMode: pick("menuBarMode", isBoolean),
    searchField: pick(
      "searchField",
      (value): value is AppSettings["searchField"] =>
        SEARCH_FIELD_OPTIONS.some((option) => option.value === value),
    ),
    pinnedPaths: Array.isArray(stored.pinnedPaths)
      ? stored.pinnedPaths.filter(
          (path): path is string => typeof path === "string",
        )
      : DEFAULT_SETTINGS.pinnedPaths,
    watchedPorts: Array.isArray(stored.watchedPorts)
      ? stored.watchedPorts.filter(isPort)
      : DEFAULT_SETTINGS.watchedPorts,
    watchedPortNotifications: pick("watchedPortNotifications", isBoolean),
    includeUdp: pick("includeUdp", isBoolean),
    useHttpsForLocalhost: pick("useHttpsForLocalhost", isBoolean),
  };

  // Hiding both kinds would leave the table empty.
  if (settings.hideSystemServices && settings.hideUserServices) {
    settings.hideUserServices = false;
  }
  return settings;
}

function readLegacySettings(): AppSettings | null {
  try {
    const raw = localStorage.getItem(LEGACY_KEY);
    return raw ? readSettings(JSON.parse(raw)) : null;
  } catch {
    return null;
  }
}

/**
 * Loads the settings before the first render. Outside the app (a plain
 * browser) there is no backend: the defaults are used and changes last for
 * the session.
 */
export async function initSettings(): Promise<void> {
  try {
    const reply = await invoke<{ settings: unknown; stored: boolean }>(
      "get_settings",
    );
    connected = true;
    let settings = readSettings(reply.settings);

    // The first launch of a version whose backend keeps the settings: hand
    // over what the window had been keeping. The window's copy is left where
    // it is, which is what an older version of the app would read.
    const legacy = reply.stored ? null : readLegacySettings();
    if (legacy) {
      settings = readSettings(
        await invoke("update_settings", { patch: legacy }),
      );
    }
    publish(settings);

    // The tray changes settings too.
    void listen("settings-changed", (event) => {
      if (unanswered === 0) {
        publish(readSettings(event.payload));
      }
    });
  } catch {
    // not inside the app
  }
}

type Change =
  Partial<AppSettings> | ((current: AppSettings) => Partial<AppSettings>);

/** Changes the settings named, on screen at once and then in the backend. */
export function updateSettings(change: Change): void {
  const patch = typeof change === "function" ? change(current) : change;
  const keys = Object.keys(patch) as (keyof AppSettings)[];
  if (keys.every((key) => Object.is(patch[key], current[key]))) {
    return;
  }

  publish({ ...current, ...patch });
  if (!connected) {
    return;
  }

  unanswered += 1;
  invoke("update_settings", { patch }).then(
    (saved) => {
      unanswered -= 1;
      if (unanswered === 0) {
        publish(readSettings(saved));
      }
    },
    async (error: unknown) => {
      unanswered -= 1;
      toast.error("Could not save settings", { description: String(error) });
      if (unanswered > 0) {
        return;
      }
      // Show what the backend actually holds.
      try {
        const reply = await invoke<{ settings: unknown }>("get_settings");
        if (unanswered === 0) {
          publish(readSettings(reply.settings));
        }
      } catch {
        // keep what is on screen
      }
    },
  );
}
