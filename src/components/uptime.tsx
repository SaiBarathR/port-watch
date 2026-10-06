import { useNowSeconds } from "@/lib/clock";
import { formatUptime, uptimeSeconds } from "@/lib/types";

/**
 * How long a process has been running. It keeps its own time, so a scan that
 * found nothing new does not have to touch the table for this to stay current.
 */
export function Uptime({ startedAt }: { startedAt: number }) {
  return formatUptime(uptimeSeconds(startedAt, useNowSeconds()));
}
