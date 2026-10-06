import {
  dismissChangeToastConfirmation,
  dismissPortChangeToasts,
} from "@/lib/change-toasts";
import { updateSettings } from "@/lib/settings-store";

// The settings changes that do more than replace one value.

// Hiding both user and system services would blank the table, so turning
// one hide-toggle on always releases the other.
export function setHideSystemServices(hide: boolean) {
  updateSettings((current) => ({
    hideSystemServices: hide,
    hideUserServices: hide ? false : current.hideUserServices,
  }));
}

export function setHideUserServices(hide: boolean) {
  updateSettings((current) => ({
    hideUserServices: hide,
    hideSystemServices: hide ? false : current.hideSystemServices,
  }));
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
