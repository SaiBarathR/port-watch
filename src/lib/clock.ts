import { useSyncExternalStore } from "react";

// One timer for every component that shows elapsed time, running only while
// something is subscribed.
const listeners = new Set<() => void>();
let timer: number | undefined;
let nowSeconds = currentSeconds();

function currentSeconds(): number {
  return Math.floor(Date.now() / 1000);
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  if (timer === undefined) {
    nowSeconds = currentSeconds();
    timer = window.setInterval(() => {
      nowSeconds = currentSeconds();
      for (const notify of listeners) {
        notify();
      }
    }, 1000);
  }

  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) {
      window.clearInterval(timer);
      timer = undefined;
    }
  };
}

/** The current time in Unix seconds, re-rendering the caller once a second. */
export function useNowSeconds(): number {
  return useSyncExternalStore(subscribe, () => nowSeconds);
}
