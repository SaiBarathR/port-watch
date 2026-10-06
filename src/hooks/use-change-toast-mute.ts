import { useEffect } from "react";
import { isChangeToastsMuted } from "@/lib/change-toasts";
import { updateSettings, useSettings } from "@/lib/settings-store";

/**
 * Drops an expired mute so the UI stops showing it. The toast gate itself
 * checks the clock on every scan.
 */
export function useChangeToastMuteExpiry() {
  const mutedUntil = useSettings().changeToastsMutedUntil;

  useEffect(() => {
    if (mutedUntil === null) {
      return;
    }

    let timer: number | undefined;
    const check = () => {
      window.clearTimeout(timer);
      const now = Date.now();
      if (isChangeToastsMuted(mutedUntil, now)) {
        timer = window.setTimeout(check, mutedUntil - now);
        return;
      }
      updateSettings((current) =>
        current.changeToastsMutedUntil === mutedUntil
          ? { changeToastsMutedUntil: null }
          : {},
      );
    };

    check();
    // Timers stall while the machine sleeps, so re-check on window focus too.
    window.addEventListener("focus", check);

    return () => {
      window.clearTimeout(timer);
      window.removeEventListener("focus", check);
    };
  }, [mutedUntil]);
}
