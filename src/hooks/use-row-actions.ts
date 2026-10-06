import { useCallback, useSyncExternalStore } from "react";
import { toast } from "sonner";
import { useProcessActions } from "@/components/process-actions";
import { commands, orToast } from "@/lib/commands";
import { getPlatform } from "@/lib/platform";
import {
  rowActions,
  type RowAction,
  type RowActionId,
} from "@/lib/row-actions";
import { togglePinnedPath } from "@/lib/settings-actions";
import { getSettings, updateSettings, useSettings } from "@/lib/settings-store";
import {
  localhostUrl,
  pinPath,
  primaryPort,
  type AppSettings,
  type PortProcess,
} from "@/lib/types";

// The key that shows a menu's alternate items: Option on a Mac, as in
// Finder, and Shift elsewhere, as in Explorer.
let alternateHeld = false;
const alternateListeners = new Set<() => void>();

function trackAlternateKey(listener: () => void) {
  const update = (event: KeyboardEvent) => {
    const held = getPlatform() === "macos" ? event.altKey : event.shiftKey;
    if (held !== alternateHeld) {
      alternateHeld = held;
      for (const notify of alternateListeners) {
        notify();
      }
    }
  };
  const release = () => update(new KeyboardEvent("keyup"));

  if (alternateListeners.size === 0) {
    window.addEventListener("keydown", update);
    window.addEventListener("keyup", update);
    window.addEventListener("blur", release);
  }
  alternateListeners.add(listener);
  return () => {
    alternateListeners.delete(listener);
    if (alternateListeners.size === 0) {
      window.removeEventListener("keydown", update);
      window.removeEventListener("keyup", update);
      window.removeEventListener("blur", release);
      alternateHeld = false;
    }
  };
}

/** Whether the key that shows alternate menu items is held right now. */
export function useAlternateKey(): boolean {
  return useSyncExternalStore(trackAlternateKey, () => alternateHeld);
}

/** A row's menu, for a menu that is open: it follows settings and the key. */
export function useRowMenu(
  process: PortProcess,
  portIsShared: boolean,
): RowAction[][] {
  const settings = useSettings();
  const alternate = useAlternateKey();
  return menuFor(process, settings, portIsShared, alternate);
}

function menuFor(
  process: PortProcess,
  settings: AppSettings,
  portIsShared: boolean,
  alternate: boolean,
): RowAction[][] {
  return rowActions(process, {
    platform: getPlatform(),
    preferredEditor: settings.preferredEditor,
    pinnedPaths: settings.pinnedPaths,
    watchedPorts: settings.watchedPorts,
    allowSystemProcessActions: settings.allowSystemProcessActions,
    portIsShared,
    alternate,
  });
}

/**
 * Does what a row action says. The menus and the keyboard both go through
 * here, so a shortcut cannot do what its menu item would refuse.
 */
export function useRowActionRunner() {
  const { stop, freePort, remove, showHistory } = useProcessActions();

  return useCallback(
    (id: RowActionId, process: PortProcess, portIsShared = false) => {
      const settings = getSettings();
      const action = menuFor(process, settings, portIsShared, id === "delete")
        .flat()
        .find((candidate) => candidate.id === id);
      if (!action) {
        return;
      }
      if (action.disabledReason) {
        toast.error(action.disabledReason);
        return;
      }

      const port = primaryPort(process);
      const folder = pinPath(process);
      const url =
        port === null ? "" : localhostUrl(port, settings.useHttpsForLocalhost);

      switch (id) {
        case "open-browser":
          void orToast(commands.openUrl(url));
          break;
        case "copy-url":
          void orToast(navigator.clipboard.writeText(url), "URL copied");
          break;
        case "open-editor":
          void orToast(commands.openEditor(folder, settings.preferredEditor));
          break;
        case "open-terminal":
          void orToast(commands.openTerminal(folder));
          break;
        case "reveal":
          void orToast(commands.revealFolder(folder));
          break;
        case "copy-path":
          void orToast(navigator.clipboard.writeText(folder), "Path copied");
          break;
        case "pin":
          togglePinnedPath(folder);
          break;
        case "watch":
          if (port !== null) {
            const watching = settings.watchedPorts.includes(port);
            updateSettings({
              watchedPorts: watching
                ? settings.watchedPorts.filter((watched) => watched !== port)
                : [...settings.watchedPorts, port].sort((a, b) => a - b),
            });
            toast.success(
              watching
                ? `No longer watching port ${port}`
                : `Watching port ${port}`,
              {
                description:
                  watching || settings.watchedPortNotifications
                    ? undefined
                    : "Turn on watched port alerts in Settings to be notified when it changes hands.",
              },
            );
          }
          break;
        case "history":
          if (port !== null) {
            showHistory(port);
          }
          break;
        case "stop":
          stop([process]);
          break;
        case "free-port":
          if (port !== null) {
            freePort(port, process);
          }
          break;
        case "trash":
          remove(process, "trash");
          break;
        case "delete":
          remove(process, "permanent");
          break;
      }
    },
    [stop, freePort, remove, showHistory],
  );
}
