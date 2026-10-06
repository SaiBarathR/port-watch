import {
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import {
  changeToastStatus,
  isChangeToastsMuted,
  showPortChanges,
} from "@/lib/change-toasts";
import {
  appendPortHistoryEvents,
  type PortHistoryEvent,
} from "@/lib/port-history";
import { filterPortProcesses, normalizePortProcess } from "@/lib/port-filter";
import { diffProcesses } from "@/lib/scan-diff";
import { updateSettings, useSettings } from "@/lib/settings-store";
import type { PortProcess, RowChangeKind } from "@/lib/types";
import { parsePort, portSignature, processHasPort } from "@/lib/types";

const CHANGE_HIGHLIGHT_MS = 10_000;

interface PortsUpdatedPayload {
  processes: PortProcess[];
  error: string | null;
  /** Goes up each time the backend's result changes. */
  revision: number;
}

function parsePortsPayload(payload: unknown): PortsUpdatedPayload {
  if (payload && typeof payload === "object") {
    const record = payload as Record<string, unknown>;
    const processes = record.processes;
    const error = record.error;

    return {
      processes: Array.isArray(processes)
        ? processes.map((item) =>
            normalizePortProcess(
              item as PortProcess & { isSystemService?: boolean },
            ),
          )
        : [],
      error: typeof error === "string" ? error : null,
      revision: typeof record.revision === "number" ? record.revision : 0,
    };
  }

  return { processes: [], error: null, revision: 0 };
}

function processesUnchanged(prev: PortProcess[], next: PortProcess[]): boolean {
  if (prev.length !== next.length) {
    return false;
  }

  const prevById = new Map(prev.map((process) => [process.id, process]));
  for (const process of next) {
    const old = prevById.get(process.id);
    if (!old || portSignature(old) !== portSignature(process)) {
      return false;
    }
  }

  return true;
}

function collectWatchedPortChanges(
  prev: PortProcess[],
  next: PortProcess[],
  watchedPorts: number[],
): { occupied: PortHistoryEvent[]; freed: PortHistoryEvent[] } {
  const occupied: PortHistoryEvent[] = [];
  const freed: PortHistoryEvent[] = [];
  const watched = new Set(watchedPorts);
  if (watched.size === 0) {
    return { occupied, freed };
  }

  const prevByPort = new Map<number, PortProcess>();
  for (const process of prev) {
    for (const binding of process.ports) {
      if (watched.has(binding.port)) {
        prevByPort.set(binding.port, process);
      }
    }
  }

  const nextByPort = new Map<number, PortProcess>();
  for (const process of next) {
    for (const binding of process.ports) {
      if (watched.has(binding.port)) {
        nextByPort.set(binding.port, process);
      }
    }
  }

  const timestamp = new Date().toISOString();

  for (const port of watched) {
    const was = prevByPort.get(port);
    const now = nextByPort.get(port);
    if (!was && now) {
      const binding = now.ports.find((item) => item.port === port)!;
      occupied.push({
        timestamp,
        kind: "occupied",
        port,
        protocol: binding.protocol,
        pid: now.pid,
        processName: now.name,
      });
    } else if (was && !now) {
      const binding = was.ports.find((item) => item.port === port)!;
      freed.push({
        timestamp,
        kind: "freed",
        port,
        protocol: binding.protocol,
        pid: was.pid,
        processName: was.name,
      });
    } else if (was && now && was.pid !== now.pid) {
      // The port changed hands between scans (e.g. a dev-server restart):
      // notify about the new occupant even though the port never appeared free.
      const binding = now.ports.find((item) => item.port === port)!;
      occupied.push({
        timestamp,
        kind: "occupied",
        port,
        protocol: binding.protocol,
        pid: now.pid,
        processName: now.name,
      });
    }
  }

  return { occupied, freed };
}

// Diff at binding granularity (pid + protocol + port) rather than whole
// processes, so a surviving process that gains or drops a port still
// produces occupied/freed events.
function collectBindings(
  processes: PortProcess[],
): Map<string, { pid: number; name: string; port: number; protocol: string }> {
  const bindings = new Map<
    string,
    { pid: number; name: string; port: number; protocol: string }
  >();
  for (const process of processes) {
    for (const binding of process.ports) {
      bindings.set(`${process.pid}|${binding.protocol}|${binding.port}`, {
        pid: process.pid,
        name: process.name,
        port: binding.port,
        protocol: binding.protocol,
      });
    }
  }
  return bindings;
}

function collectHistoryEvents(
  prev: PortProcess[],
  next: PortProcess[],
): PortHistoryEvent[] {
  const events: PortHistoryEvent[] = [];
  const timestamp = new Date().toISOString();
  const prevBindings = collectBindings(prev);
  const nextBindings = collectBindings(next);

  for (const [key, binding] of nextBindings) {
    if (!prevBindings.has(key)) {
      events.push({
        timestamp,
        kind: "occupied",
        port: binding.port,
        protocol: binding.protocol,
        pid: binding.pid,
        processName: binding.name,
      });
    }
  }

  for (const [key, binding] of prevBindings) {
    if (!nextBindings.has(key)) {
      events.push({
        timestamp,
        kind: "freed",
        port: binding.port,
        protocol: binding.protocol,
        pid: binding.pid,
        processName: binding.name,
      });
    }
  }

  return events;
}

export function usePortScan() {
  const [processes, setProcesses] = useState<PortProcess[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const settings = useSettings();
  const [search, setSearch] = useState("");
  const [rowChanges, setRowChanges] = useState<Map<string, RowChangeKind>>(
    () => new Map(),
  );
  const previousProcessesRef = useRef<PortProcess[]>([]);
  const isInitialScanRef = useRef(true);
  const changeClearTimerRef = useRef<number | null>(null);
  const settingsRef = useRef(settings);
  const applyScanResultRef = useRef<
    (result: PortProcess[], scanError: string | null) => void
  >(() => {});

  useEffect(() => {
    settingsRef.current = settings;
  }, [settings]);

  // Replies to requests and scan events travel separately, so a reply can
  // arrive after the event that superseded it. Applying it would roll the
  // table back and log changes that never happened.
  const lastRevisionRef = useRef(-1);
  const applyPayload = useCallback((payload: PortsUpdatedPayload) => {
    if (payload.revision < lastRevisionRef.current) {
      return;
    }
    lastRevisionRef.current = payload.revision;
    applyScanResultRef.current(payload.processes, payload.error);
  }, []);

  const setRefreshPaused = useCallback((paused: boolean) => {
    void invoke("set_refresh_paused", { paused }).catch(() => {
      // ignore outside Tauri
    });
  }, []);

  const scheduleChangeClear = useCallback(() => {
    if (changeClearTimerRef.current !== null) {
      window.clearTimeout(changeClearTimerRef.current);
    }
    changeClearTimerRef.current = window.setTimeout(() => {
      setRowChanges(new Map());
      changeClearTimerRef.current = null;
    }, CHANGE_HIGHLIGHT_MS);
  }, []);

  const applyScanResult = useCallback(
    (result: PortProcess[], scanError: string | null) => {
      if (scanError) {
        setError(scanError);
        setRefreshing(false);
        setLoading(false);
        return;
      }

      setError(null);
      const normalized = result.map((item) =>
        normalizePortProcess(
          item as PortProcess & { isSystemService?: boolean },
        ),
      );
      const currentSettings = settingsRef.current;
      const prev = previousProcessesRef.current;

      if (!isInitialScanRef.current && processesUnchanged(prev, normalized)) {
        // Same processes and bindings, but uptime/name/cwd may have moved on:
        // still publish the fresh data, just skip the diff/toast/history work.
        previousProcessesRef.current = normalized;
        setProcesses(normalized);
        setRefreshing(false);
        setLoading(false);
        return;
      }

      if (!isInitialScanRef.current) {
        // Rows and toasts follow what the table shows. History and watched
        // ports below still see every process.
        const inView = (list: PortProcess[]) =>
          filterPortProcesses(
            list,
            currentSettings.hideSystemServices,
            currentSettings.hideUserServices,
            "",
            "all",
          );
        const { rowChanges: nextChanges, messages } = diffProcesses(
          inView(prev),
          inView(normalized),
        );

        if (nextChanges.size > 0) {
          setRowChanges(nextChanges);
          scheduleChangeClear();
        }

        // A window closed to the tray or minimized cannot show a toast; it
        // would only be held back and shown, stale, when the window returns.
        if (
          messages.length > 0 &&
          document.visibilityState !== "hidden" &&
          changeToastStatus(currentSettings, Date.now()) === "on"
        ) {
          showPortChanges(messages);
        }

        appendPortHistoryEvents(collectHistoryEvents(prev, normalized));

        const watchedChanges = collectWatchedPortChanges(
          prev,
          normalized,
          currentSettings.watchedPorts,
        );
        if (currentSettings.watchedPortNotifications) {
          for (const event of [
            ...watchedChanges.occupied,
            ...watchedChanges.freed,
          ]) {
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
      }

      isInitialScanRef.current = false;
      previousProcessesRef.current = normalized;
      setProcesses(normalized);
      setRefreshing(false);
      setLoading(false);
    },
    [scheduleChangeClear],
  );

  useEffect(() => {
    applyScanResultRef.current = applyScanResult;
  }, [applyScanResult]);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    try {
      // Resolves with the result when a scan that started after this call
      // has finished. An event only follows if that scan found something
      // new, so the reply is what ends the spinner.
      applyPayload(
        parsePortsPayload(
          await invoke<PortsUpdatedPayload>("trigger_port_scan"),
        ),
      );
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
  }, [applyPayload]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    // Once a live poller event has been applied, the startup snapshot below is
    // stale — applying it afterwards would reverse-diff fresh data and produce
    // phantom freed/occupied toasts and history entries.
    let receivedLiveEvent = false;

    void (async () => {
      try {
        const stop = await listen<PortsUpdatedPayload>(
          "ports-updated",
          (event) => {
            receivedLiveEvent = true;
            applyPayload(parsePortsPayload(event.payload));
          },
        );

        if (cancelled) {
          stop();
          return;
        }
        unlisten = stop;

        const payload = parsePortsPayload(
          await invoke<PortsUpdatedPayload>("get_listening_ports"),
        );

        if (cancelled || receivedLiveEvent) {
          return;
        }

        applyPayload(payload);
      } catch (err) {
        if (cancelled) {
          return;
        }
        setError(err instanceof Error ? err.message : String(err));
        setLoading(false);
        setRefreshing(false);
      }
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [applyPayload]);

  useEffect(() => {
    return () => {
      if (changeClearTimerRef.current !== null) {
        window.clearTimeout(changeClearTimerRef.current);
      }
    };
  }, []);

  const userCount = processes.filter((p) => !p.is_system_service).length;
  const systemCount = processes.filter((p) => p.is_system_service).length;

  // Drop an expired mute so the UI stops showing it; the toast gate itself
  // checks the clock on every scan.
  const mutedUntil = settings.changeToastsMutedUntil;
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

  const deferredSearch = useDeferredValue(search);
  const filtered = useMemo(
    () =>
      filterPortProcesses(
        processes,
        settings.hideSystemServices,
        settings.hideUserServices,
        deferredSearch,
        settings.searchField,
      ),
    [
      processes,
      settings.hideSystemServices,
      settings.hideUserServices,
      settings.searchField,
      deferredSearch,
    ],
  );

  const exactPortQuery = useMemo(() => {
    if (settings.searchField !== "port") {
      return null;
    }
    return parsePort(search);
  }, [search, settings.searchField]);

  const portLookupEmpty =
    exactPortQuery !== null &&
    !loading &&
    !processes.some((p) => processHasPort(p, exactPortQuery));

  const portLookupOccupants = useMemo(() => {
    if (exactPortQuery === null) {
      return [];
    }
    return processes.filter((process) =>
      processHasPort(process, exactPortQuery),
    );
  }, [exactPortQuery, processes]);

  return {
    processes: filtered,
    allProcesses: processes,
    loading,
    refreshing,
    error,
    refresh,
    search,
    setSearch,
    portLookupEmpty,
    exactPortQuery,
    portLookupOccupants,
    settings,
    setRefreshPaused,
    rowChanges,
    userCount,
    systemCount,
    hiddenSystemCount: settings.hideSystemServices ? systemCount : 0,
    hiddenUserCount: settings.hideUserServices ? userCount : 0,
  };
}
