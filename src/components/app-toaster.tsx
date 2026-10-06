import { useState } from "react";
import { Toaster, useSonner } from "sonner";
import { Button } from "@/components/ui/button";
import {
  MUTE_DURATIONS,
  VISIBLE_TOASTS,
  dismissAllToasts,
  isPortChangeToastId,
  muteChangeToasts,
  turnOffChangeToasts,
} from "@/lib/change-toasts";
import {
  setChangeToastsMutedUntil,
  setShowChangeToasts,
} from "@/lib/settings-actions";

// Matches sonner's default viewport offset and gap between toasts.
const VIEWPORT_OFFSET_PX = 24;
const TOAST_GAP_PX = 14;
const CONTROLS_HEIGHT_PX = 36;

interface AppToasterProps {
  theme: "light" | "dark";
}

export function AppToaster({ theme }: AppToasterProps) {
  const { toasts } = useSonner();
  // Keeps the controls in place while the pointer is on them, so a toast that
  // expires mid-click cannot hand the click to the table underneath.
  const [pointerInside, setPointerInside] = useState(false);
  const hasPortChangeToasts = toasts.some((item) =>
    isPortChangeToastId(item.id),
  );
  const showControls = hasPortChangeToasts || pointerInside;

  const run = (action: () => void) => () => {
    setPointerInside(false);
    action();
  };

  return (
    <>
      {showControls && (
        <div
          role="toolbar"
          aria-label="Port change toast controls"
          // Above sonner's own z-index: a toast leaving the stack slides down
          // across this spot and would otherwise take the pointer with it.
          // pointer-events-auto keeps it usable while a modal dialog is open.
          className="pointer-events-auto fixed right-6 bottom-6 z-[1000000000] flex w-[356px] items-center justify-between rounded-lg border bg-popover px-1 text-xs text-popover-foreground shadow-md"
          style={{ height: CONTROLS_HEIGHT_PX }}
          onPointerEnter={() => setPointerInside(true)}
          onPointerLeave={() => setPointerInside(false)}
        >
          <Button
            variant="ghost"
            size="sm"
            className="h-7 px-2 text-xs"
            title="Dismiss all notifications"
            onClick={run(dismissAllToasts)}
          >
            Clear all
          </Button>
          <div className="flex items-center gap-0.5">
            <span className="px-1 text-muted-foreground">Mute for</span>
            {MUTE_DURATIONS.map((duration) => (
              <Button
                key={duration.ms}
                variant="ghost"
                size="sm"
                className="h-7 px-1.5 text-xs"
                title={`Mute port change toasts for ${duration.label}`}
                aria-label={`Mute port change toasts for ${duration.label}`}
                onClick={run(() =>
                  muteChangeToasts(duration.ms, setChangeToastsMutedUntil),
                )}
              >
                {duration.shortLabel}
              </Button>
            ))}
          </div>
          <Button
            variant="ghost"
            size="sm"
            className="h-7 px-2 text-xs"
            title="Turn off port change toasts"
            onClick={run(() => turnOffChangeToasts(setShowChangeToasts))}
          >
            Turn off
          </Button>
        </div>
      )}

      <Toaster
        richColors
        closeButton
        expand
        position="bottom-right"
        duration={8000}
        visibleToasts={VISIBLE_TOASTS}
        theme={theme}
        offset={{
          bottom: showControls
            ? VIEWPORT_OFFSET_PX + CONTROLS_HEIGHT_PX + TOAST_GAP_PX
            : VIEWPORT_OFFSET_PX,
        }}
      />
    </>
  );
}
