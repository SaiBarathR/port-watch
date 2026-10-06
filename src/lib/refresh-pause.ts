import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

// Periodic scans stop while something on screen should not move under the
// pointer: an open row menu, a dialog, a column being dragged. Each holds its
// own reason, and scans resume when the last one lets go. They used to set
// and clear one shared flag in turn, so closing a dialog could resume scans
// under a menu that was still open.
const reasons = new Set<string>();

let told = false;
let scheduled = false;

// At the end of the turn, and only when the answer changed: a menu that
// gives way to a dialog lets go and takes hold within one render, and the
// backend scans as soon as it hears "resume".
function tellBackend() {
  if (scheduled) {
    return;
  }
  scheduled = true;
  queueMicrotask(() => {
    scheduled = false;
    const paused = reasons.size > 0;
    if (paused === told) {
      return;
    }
    told = paused;
    void invoke("set_refresh_paused", { paused }).catch(() => {
      // not inside the app
    });
  });
}

/** Pauses periodic scans until the function it returns is called. */
export function holdRefresh(reason: string): () => void {
  reasons.add(reason);
  tellBackend();

  return () => {
    if (reasons.delete(reason)) {
      tellBackend();
    }
  };
}

/** Pauses periodic scans for as long as `active` is true. */
export function useRefreshPause(reason: string, active: boolean): void {
  useEffect(() => (active ? holdRefresh(reason) : undefined), [reason, active]);
}
