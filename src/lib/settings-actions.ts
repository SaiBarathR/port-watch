import {
  dismissChangeToastConfirmation,
  dismissPortChangeToasts,
} from "@/lib/change-toasts";
import { updateSettings } from "@/lib/settings-store";
import type { AppSettings } from "@/lib/types";

// The settings changes that do more than replace one value.

/** Which listeners the table shows. */
export type ListenerScope = "user" | "system" | "all";

// Stored as two "hide" switches, which is how earlier versions asked. They
// were never two choices: hiding both left an empty table, so turning one on
// turned the other off.
export function listenerScope(
  settings: Pick<AppSettings, "hideSystemServices" | "hideUserServices">,
): ListenerScope {
  if (settings.hideSystemServices) {
    return "user";
  }
  return settings.hideUserServices ? "system" : "all";
}

export function setListenerScope(scope: ListenerScope) {
  updateSettings({
    hideSystemServices: scope === "user",
    hideUserServices: scope === "system",
  });
}

export function setShowChangeToasts(showChangeToasts: boolean) {
  dismissChangeToastConfirmation();
  // Turning toasts on means on, so a mute never outlives the switch.
  updateSettings({ showChangeToasts, changeToastsMutedUntil: null });
  if (!showChangeToasts) {
    dismissPortChangeToasts();
  }
}

export function setChangeToastsMutedUntil(
  changeToastsMutedUntil: number | null,
) {
  dismissChangeToastConfirmation();
  updateSettings({ changeToastsMutedUntil });
  if (changeToastsMutedUntil !== null) {
    dismissPortChangeToasts();
  }
}

export function togglePinnedPath(path: string) {
  updateSettings((current) => ({
    pinnedPaths: current.pinnedPaths.includes(path)
      ? current.pinnedPaths.filter((item) => item !== path)
      : [...current.pinnedPaths, path],
  }));
}
