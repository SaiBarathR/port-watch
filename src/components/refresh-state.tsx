import { useNowSeconds } from "@/lib/clock";
import { useRefreshPaused } from "@/lib/refresh-pause";
import type { RefreshInterval } from "@/lib/types";
import { cn } from "@/lib/utils";

/** "just now", "40 s ago", "3 min ago", "2 h ago". */
export function formatAgo(seconds: number): string {
  if (seconds < 5) {
    return "just now";
  }
  if (seconds < 60) {
    return `${seconds} s ago`;
  }
  if (seconds < 3600) {
    return `${Math.floor(seconds / 60)} min ago`;
  }
  return `${Math.floor(seconds / 3600)} h ago`;
}

// On its own so that only this text is drawn again every second.
function UpdatedAgo({ at }: { at: number }) {
  const now = useNowSeconds();
  return <>updated {formatAgo(Math.max(0, now - Math.floor(at / 1000)))}</>;
}

interface RefreshStateProps {
  intervalMs: RefreshInterval;
  /** True until the first scan has come back, whether or not it worked. */
  firstScanPending: boolean;
  /** When a scan last confirmed the list, in ms; null if none has yet. */
  lastScanAt: number | null;
  includeUdp: boolean;
}

/**
 * Whether the list is being kept up to date. Scans pause while a menu or a
 * dialog is open, and can be turned off; neither used to show anywhere.
 */
export function RefreshState({
  intervalMs,
  firstScanPending,
  lastScanAt,
  includeUdp,
}: RefreshStateProps) {
  const paused = useRefreshPaused();
  // "Scanning" only while a scan is actually awaited: a first scan that
  // failed is over, and the banner above the table says what went wrong.
  const state = firstScanPending
    ? "scanning"
    : paused
      ? "paused"
      : intervalMs === 0
        ? "off"
        : "live";

  return (
    <span className="flex min-w-0 items-center gap-1.5">
      <span
        aria-hidden
        className={cn(
          "size-1.5 shrink-0 rounded-full",
          state === "live" && "bg-emerald-500",
          state === "paused" && "bg-amber-500",
          (state === "off" || state === "scanning") && "bg-muted-foreground/50",
        )}
      />
      <span className="truncate">
        {state === "scanning" && "Scanning ports…"}
        {state === "paused" && "Paused while a menu or dialog is open"}
        {state === "live" && `Live · every ${intervalMs / 1000} s`}
        {state === "off" && "Auto-refresh off"}
        {state === "off" && lastScanAt !== null && (
          <>
            {" · "}
            <UpdatedAgo at={lastScanAt} />
          </>
        )}
        {includeUdp && state !== "scanning" && " · TCP and UDP"}
      </span>
    </span>
  );
}
