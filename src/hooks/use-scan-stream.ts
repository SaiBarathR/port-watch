import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { shareUnchanged } from "@/lib/scan-diff";
import type { PortProcess } from "@/lib/types";

interface PortsUpdatedPayload {
  processes: PortProcess[];
  error: string | null;
  /** Goes up each time the backend's result changes. */
  revision: number;
}

function readPayload(payload: unknown): PortsUpdatedPayload {
  const record =
    payload && typeof payload === "object"
      ? (payload as Record<string, unknown>)
      : {};

  return {
    processes: Array.isArray(record.processes)
      ? (record.processes as PortProcess[])
      : [],
    error: typeof record.error === "string" ? record.error : null,
    revision: typeof record.revision === "number" ? record.revision : 0,
  };
}

/**
 * The backend's scans as they arrive: the list at launch, an event whenever
 * a scan finds something different, and the reply to a manual refresh.
 * `onChange` is called with the lists before and after, from the second
 * result on; the first is what everything else is compared with.
 */
export function useScanStream(
  onChange: (prev: PortProcess[], next: PortProcess[]) => void,
) {
  const [processes, setProcesses] = useState<PortProcess[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // When the list on screen was last confirmed by a scan, in ms.
  const [lastScanAt, setLastScanAt] = useState<number | null>(null);

  const processesRef = useRef<PortProcess[]>([]);
  const hasResultRef = useRef(false);
  const onChangeRef = useRef(onChange);
  useEffect(() => {
    onChangeRef.current = onChange;
  }, [onChange]);

  // Replies to requests and scan events travel separately, so a reply can
  // arrive after the event that superseded it. Applying it would roll the
  // table back and log changes that never happened.
  const lastRevisionRef = useRef(-1);
  const apply = useCallback((payload: PortsUpdatedPayload) => {
    if (payload.revision < lastRevisionRef.current) {
      return;
    }
    lastRevisionRef.current = payload.revision;
    setLoading(false);

    // A failed scan keeps the last list on screen.
    if (payload.error) {
      setError(payload.error);
      return;
    }
    setError(null);
    setLastScanAt(Date.now());

    const prev = processesRef.current;
    const next = shareUnchanged(prev, payload.processes);
    if (hasResultRef.current && next !== prev) {
      onChangeRef.current(prev, next);
    }
    hasResultRef.current = true;
    processesRef.current = next;
    setProcesses(next);
  }, []);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    try {
      // Resolves with the result when a scan that started after this call
      // has finished. An event only follows if that scan found something
      // new, so the reply is what ends the spinner.
      apply(readPayload(await invoke("trigger_port_scan")));
    } catch (err) {
      // The request failed, which is not the same as a scan failing. The
      // error banner shows what the backend reports and is cleared by it;
      // nothing would ever clear this one.
      toast.error("Refresh failed", {
        description: err instanceof Error ? err.message : String(err),
      });
      setLoading(false);
    } finally {
      setRefreshing(false);
    }
  }, [apply]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    // Once a live event has been applied, the list asked for at startup is
    // older than what is on screen.
    let receivedLiveEvent = false;

    void (async () => {
      try {
        const stop = await listen("ports-updated", (event) => {
          receivedLiveEvent = true;
          apply(readPayload(event.payload));
        });

        if (cancelled) {
          stop();
          return;
        }
        unlisten = stop;

        const payload = readPayload(await invoke("get_listening_ports"));
        if (cancelled || receivedLiveEvent) {
          return;
        }
        apply(payload);
      } catch (err) {
        if (cancelled) {
          return;
        }
        setError(err instanceof Error ? err.message : String(err));
        setLoading(false);
      }
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [apply]);

  return { processes, loading, refreshing, error, refresh, lastScanAt };
}
