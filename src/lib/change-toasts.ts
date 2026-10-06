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
const PORT_CHANGE_TOAST_ID = "port-changes";
const PORT_CHANGE_PREVIEW_LINES = 5;

let confirmationToastId: number | string | null = null;

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

export function isPortChangeToastId(id: number | string): boolean {
  return id === PORT_CHANGE_TOAST_ID;
}

/** The changes the port change toast on screen stands for. */
export interface ShownPortChanges {
  total: number;
  /** The most recent lines, oldest first. */
  latest: string[];
}

const NO_PORT_CHANGES: ShownPortChanges = { total: 0, latest: [] };

let shownPortChanges = NO_PORT_CHANGES;

export function addPortChanges(
  shown: ShownPortChanges,
  messages: string[],
): ShownPortChanges {
  return {
    total: shown.total + messages.length,
    latest: [...shown.latest, ...messages].slice(-PORT_CHANGE_PREVIEW_LINES),
  };
}

export function portChangeToastContent(shown: ShownPortChanges): {
  title: string;
  description: string;
} {
  const earlier = shown.total - shown.latest.length;
  return {
    title:
      shown.total === 1
        ? "Port change detected"
        : `${shown.total} port changes detected`,
    description: [
      ...(earlier > 0 ? [`+${earlier} earlier`] : []),
      ...shown.latest,
    ].join("\n"),
  };
}

/**
 * Announces port changes in a single toast. While that toast is on screen,
 * later changes are added to it, instead of every scan stacking a new one.
 */
export function showPortChanges(messages: string[]) {
  shownPortChanges = addPortChanges(shownPortChanges, messages);
  const forget = () => {
    shownPortChanges = NO_PORT_CHANGES;
  };
  const { title, description } = portChangeToastContent(shownPortChanges);
  toast.info(title, {
    id: PORT_CHANGE_TOAST_ID,
    description,
    duration: 12_000,
    onDismiss: forget,
    onAutoClose: forget,
  });
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

// A confirmation's Unmute / Undo is only right for the state it announced, so
// any later change to that state takes the confirmation down first.
export function dismissChangeToastConfirmation() {
  if (confirmationToastId !== null) {
    toast.dismiss(confirmationToastId);
    confirmationToastId = null;
  }
}

export function muteChangeToasts(
  durationMs: number,
  onMutedUntilChange: (mutedUntil: number | null) => void,
) {
  const mutedUntil = Date.now() + durationMs;
  onMutedUntilChange(mutedUntil);
  confirmationToastId = toast("Port change toasts muted", {
    description: `Until ${formatMutedUntil(mutedUntil)}.`,
    action: { label: "Unmute", onClick: () => onMutedUntilChange(null) },
  });
}

export function turnOffChangeToasts(onShowChange: (show: boolean) => void) {
  onShowChange(false);
  confirmationToastId = toast("Port change toasts turned off", {
    description: "Turn them back on from the bell menu or Settings.",
    action: { label: "Undo", onClick: () => onShowChange(true) },
  });
}
