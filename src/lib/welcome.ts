import { useSyncExternalStore } from "react";
import { dismissAllToasts } from "@/lib/change-toasts";

// The guide to the app: whether it is up, and at which page. Held here and
// not in a component, because three things that share no parent need it: the
// toolbar's menu opens it, a key opens it, and a scan must not put a toast
// over it.

export const WELCOME_PAGE_IDS = [
  "list",
  "find",
  "act",
  "stop",
  "track",
  "beyond",
  "keys",
] as const;

export type WelcomePageId = (typeof WELCOME_PAGE_IDS)[number];

/** Set once "Don't show this again" is ticked. */
export const WELCOME_HIDDEN_KEY = "port-watch-welcome-hidden";

/**
 * Whether the guide is put up when the app opens. Nothing stored means it
 * is: the launch with nothing stored is the first, which is the one it is
 * for. Where nothing can be stored it stays away, since the box that would
 * send it away could not be kept either.
 */
export function opensWithWelcome(): boolean {
  try {
    return localStorage.getItem(WELCOME_HIDDEN_KEY) !== "1";
  } catch {
    return false;
  }
}

export function setOpensWithWelcome(opens: boolean): void {
  try {
    if (opens) {
      localStorage.removeItem(WELCOME_HIDDEN_KEY);
    } else {
      localStorage.setItem(WELCOME_HIDDEN_KEY, "1");
    }
  } catch {
    // storage is unavailable: the choice lasts for this run
  }
}

interface WelcomeState {
  open: boolean;
  page: WelcomePageId;
}

// Read on first use, not when the module loads: by then the launch has had
// its chance to say the guide should stay away.
let state: WelcomeState | null = null;
const watchers = new Set<() => void>();

export function getWelcome(): WelcomeState {
  state ??= { open: opensWithWelcome(), page: WELCOME_PAGE_IDS[0] };
  return state;
}

function change(next: WelcomeState) {
  state = next;
  for (const watcher of watchers) {
    watcher();
  }
}

export function useWelcome(): WelcomeState {
  return useSyncExternalStore((watcher) => {
    watchers.add(watcher);
    return () => watchers.delete(watcher);
  }, getWelcome);
}

/** Whether the guide is over the window right now. */
export function isWelcomeOpen(): boolean {
  return getWelcome().open;
}

/** Puts the guide up at `page`. Already up, it turns to that page. */
export function showWelcome(page: WelcomePageId = WELCOME_PAGE_IDS[0]): void {
  // The toasts sit in the corner its buttons are in.
  dismissAllToasts();
  change({ open: true, page });
}

export function closeWelcome(): void {
  if (getWelcome().open) {
    change({ ...getWelcome(), open: false });
  }
}

export function turnWelcomeTo(page: WelcomePageId): void {
  if (getWelcome().page !== page) {
    change({ ...getWelcome(), page });
  }
}

/** One page on (`1`) or back (`-1`). Stays put at either end. */
export function stepWelcome(by: 1 | -1): void {
  const next =
    WELCOME_PAGE_IDS[WELCOME_PAGE_IDS.indexOf(getWelcome().page) + by];
  if (next) {
    turnWelcomeTo(next);
  }
}
