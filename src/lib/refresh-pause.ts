import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

// Periodic scans stop while something on screen should not move under the
// pointer: an open row menu, a dialog, a column being dragged. Each holds its
// own reason, and scans resume when the last one lets go. They used to set
// and clear one shared flag in turn, so closing a dialog could resume scans
// under a menu that was still open.
const reasons = new Set<string>();

function tellBackend() {
  void invoke("set_refresh_paused", { paused: reasons.size > 0 }).catch(() => {
    // not inside the app
  });
}

/** Pauses periodic scans until the function it returns is called. */
export function holdRefresh(reason: string): () => void {
  const wasPaused = reasons.size > 0;
  reasons.add(reason);
  if (!wasPaused) {
    tellBackend();
  }

  return () => {
    if (reasons.delete(reason) && reasons.size === 0) {
      tellBackend();
    }
  };
}

/** Pauses periodic scans for as long as `active` is true. */
export function useRefreshPause(reason: string, active: boolean): void {
  useEffect(() => (active ? holdRefresh(reason) : undefined), [reason, active]);
}
