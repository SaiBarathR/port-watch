import { toast } from "sonner";
import type { AppSettings } from "@/lib/types";

export const MUTE_DURATIONS = [
  { ms: 15 * 60_000, label: "15 minutes", shortLabel: "15m" },
  { ms: 60 * 60_000, label: "1 hour", shortLabel: "1h" },
  { ms: 8 * 60 * 60_000, label: "8 hours", shortLabel: "8h" },
] as const;

// The minute of slack keeps a small backward clock correction from dropping
// the longest mute right after it is set.
const MAX_MUTE_MS =
  Math.max(...MUTE_DURATIONS.map((duration) => duration.ms)) + 60_000;
const PORT_CHANGE_TOAST_ID_PREFIX = "port-change-";

let portChangeToastCount = 0;

export type ChangeToastStatus = "on" | "muted" | "off";

// A deadline further out than the longest mute on offer can only come from a
// clock change or a corrupted setting, so it does not count as muted.
export function isChangeToastsMuted(
  mutedUntil: unknown,
  now: number,
): mutedUntil is number {
  return (
    typeof mutedUntil === "number" &&
    mutedUntil > now &&
    mutedUntil - now <= MAX_MUTE_MS
  );
}

export function changeToastStatus(
  settings: Pick<AppSettings, "showChangeToasts" | "changeToastsMutedUntil">,
  now: number,
): ChangeToastStatus {
  if (!settings.showChangeToasts) {
    return "off";
  }
  return isChangeToastsMuted(settings.changeToastsMutedUntil, now)
    ? "muted"
    : "on";
}

export function formatMutedUntil(mutedUntil: number): string {
  return new Date(mutedUntil).toLocaleTimeString([], {
    hour: "numeric",
    minute: "2-digit",
  });
}

export function nextPortChangeToastId(): string {
  portChangeToastCount += 1;
  return `${PORT_CHANGE_TOAST_ID_PREFIX}${portChangeToastCount}`;
}

export function isPortChangeToastId(id: number | string): boolean {
  return typeof id === "string" && id.startsWith(PORT_CHANGE_TOAST_ID_PREFIX);
}

// Dismisses by id: sonner's no-argument dismiss() walks its whole history and
// leaves getToasts() reporting the dismissed toasts as still active.
export function dismissAllToasts() {
  for (const item of toast.getToasts()) {
    toast.dismiss(item.id);
  }
}

export function dismissPortChangeToasts() {
  for (const item of toast.getToasts()) {
    if (isPortChangeToastId(item.id)) {
      toast.dismiss(item.id);
    }
  }
}

export function muteChangeToasts(
  durationMs: number,
  onMutedUntilChange: (mutedUntil: number | null) => void,
) {
  const mutedUntil = Date.now() + durationMs;
  onMutedUntilChange(mutedUntil);
  toast("Port change toasts muted", {
    description: `Until ${formatMutedUntil(mutedUntil)}.`,
    action: { label: "Unmute", onClick: () => onMutedUntilChange(null) },
  });
}

export function turnOffChangeToasts(onShowChange: (show: boolean) => void) {
  onShowChange(false);
  toast("Port change toasts turned off", {
    description: "Turn them back on from the bell menu or Settings.",
    action: { label: "Undo", onClick: () => onShowChange(true) },
  });
}
