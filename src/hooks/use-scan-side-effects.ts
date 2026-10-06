import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { changeToastStatus, showPortChanges } from "@/lib/change-toasts";
import { isShownByScope } from "@/lib/port-filter";
import { appendPortHistoryEvents } from "@/lib/port-history";
import { diffScans } from "@/lib/scan-diff";
import { getSettings } from "@/lib/settings-store";
import type { PortProcess, RowChangeKind } from "@/lib/types";

const HIGHLIGHT_MS = 10_000;

interface Highlight {
  kind: RowChangeKind;
  /** When the highlight comes off, in ms since the epoch. */
  until: number;
}

/**
 * What a change between two scans sets off: row highlights, the toast, the
 * port history and desktop alerts for watched ports.
 */
export function useScanSideEffects() {
  const [highlights, setHighlights] = useState<Map<string, Highlight>>(
    () => new Map(),
  );

  const onScanChange = useCallback(
    (prev: PortProcess[], next: PortProcess[]) => {
      const settings = getSettings();
      const now = Date.now();
      const diff = diffScans(prev, next, {
        isShown: (process) => isShownByScope(process, settings),
        watchedPorts: settings.watchedPorts,
        timestamp: new Date(now).toISOString(),
      });

      if (diff.rowChanges.size > 0) {
        setHighlights((current) => {
          const merged = new Map(current);
          for (const [id, kind] of diff.rowChanges) {
            merged.set(id, { kind, until: now + HIGHLIGHT_MS });
          }
          return merged;
        });
      }

      // A window closed to the tray or minimized cannot show a toast; it
      // would only be held back and shown, stale, when the window returns.
      if (
        diff.messages.length > 0 &&
        document.visibilityState !== "hidden" &&
        changeToastStatus(settings, now) === "on"
      ) {
        showPortChanges(diff.messages);
      }

      appendPortHistoryEvents(diff.historyEvents);

      if (settings.watchedPortNotifications) {
        for (const event of diff.watchedEvents) {
          const title =
            event.kind === "occupied"
              ? `Port ${event.port} is now in use`
              : `Port ${event.port} is free`;
          const message = `${event.processName} (PID ${event.pid})`;
          void invoke("send_notification", { title, message }).catch(() => {
            // notifications may be unavailable
          });
        }
      }
    },
    [],
  );

  // Each row's highlight runs its own ten seconds. One timer for all of them
  // used to restart with every change, and a new change replaced the earlier
  // highlights outright.
  useEffect(() => {
    if (highlights.size === 0) {
      return;
    }
    const soonest = Math.min(
      ...[...highlights.values()].map((highlight) => highlight.until),
    );
    const timer = window.setTimeout(
      () => {
        const now = Date.now();
        setHighlights(
          (current) =>
            new Map(
              [...current].filter(([, highlight]) => highlight.until > now),
            ),
        );
      },
      Math.max(0, soonest - Date.now()),
    );
    return () => window.clearTimeout(timer);
  }, [highlights]);

  const rowChanges = useMemo(
    () =>
      new Map(
        [...highlights].map(([id, highlight]) => [id, highlight.kind] as const),
      ),
    [highlights],
  );

  return { rowChanges, onScanChange };
}
