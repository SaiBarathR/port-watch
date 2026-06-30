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
  appendPortHistoryEvents,
  type PortHistoryEvent,
} from "@/lib/port-history";
import type {
  AppSettings,
  PortProcess,
  RefreshInterval,
  RowChangeKind,
  SearchField,
} from "@/lib/types";
import {
  DEFAULT_SETTINGS,
  portSignature,
  processHasPort,
} from "@/lib/types";

const SETTINGS_KEY = "port-watch-settings";
const CHANGE_HIGHLIGHT_MS = 10_000;

interface PortsUpdatedPayload {
  processes: PortProcess[];
  error: string | null;
  scanning?: boolean;
}

import {
  filterPortProcesses,
  normalizePortProcess,
} from "@/lib/port-filter";

function parsePortsPayload(payload: unknown): PortsUpdatedPayload {
  if (Array.isArray(payload)) {
    return {
      processes: payload.map((item) =>
        normalizePortProcess(item as PortProcess & { isSystemService?: boolean }),
      ),
      error: null,
    };
  }

  if (payload && typeof payload === "object") {
    const record = payload as Record<string, unknown>;
    const processes = record.processes;
    const error = record.error;

    return {
      processes: Array.isArray(processes)
        ? processes.map((item) =>
            normalizePortProcess(item as PortProcess & { isSystemService?: boolean }),
          )
        : [],
      error: typeof error === "string" ? error : null,
      scanning: record.scanning === true,
    };
  }

  return { processes: [], error: null };
}

function loadSettings(): AppSettings {
  try {
    const raw = localStorage.getItem(SETTINGS_KEY);
    if (raw) {
      const merged = { ...DEFAULT_SETTINGS, ...JSON.parse(raw) } as AppSettings;
      if (merged.hideUserServices && merged.hideSystemServices) {
        merged.hideUserServices = false;
        localStorage.setItem(SETTINGS_KEY, JSON.stringify(merged));
      }
      return merged;
    }
  } catch {
    // ignore
  }
  return DEFAULT_SETTINGS;
}

function processesUnchanged(prev: PortProcess[], next: PortProcess[]): boolean {
  if (prev.length !== next.length) {
    return false;
  }

  const prevByPid = new Map(prev.map((process) => [process.pid, process]));
  for (const process of next) {
    const old = prevByPid.get(process.pid);
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
    }
  }

  return { occupied, freed };
}

function collectHistoryEvents(
  prev: PortProcess[],
  next: PortProcess[],
): PortHistoryEvent[] {
  const events: PortHistoryEvent[] = [];
  const timestamp = new Date().toISOString();
  const prevByPid = new Map(prev.map((process) => [process.pid, process]));

  for (const process of next) {
    const old = prevByPid.get(process.pid);
    if (!old) {
      for (const binding of process.ports) {
        events.push({
          timestamp,
          kind: "occupied",
          port: binding.port,
          protocol: binding.protocol,
          pid: process.pid,
          processName: process.name,
        });
      }
    }
    prevByPid.delete(process.pid);
  }

  for (const [, gone] of prevByPid) {
    for (const binding of gone.ports) {
      events.push({
        timestamp,
        kind: "freed",
        port: binding.port,
        protocol: binding.protocol,
        pid: gone.pid,
        processName: gone.name,
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
  const [settings, setSettings] = useState<AppSettings>(loadSettings);
  const [search, setSearch] = useState("");
  const [rowChanges, setRowChanges] = useState<Map<number, RowChangeKind>>(
    () => new Map(),
  );
  const refreshPausedRef = useRef(false);
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

  const setRefreshPaused = useCallback((paused: boolean) => {
    refreshPausedRef.current = paused;
    void invoke("set_refresh_paused", { paused }).catch(() => {
      // ignore outside Tauri
    });
  }, []);

  const persistSettings = useCallback(
    (updater: AppSettings | ((current: AppSettings) => AppSettings)) => {
      setSettings((current) => {
        const next =
          typeof updater === "function"
            ? updater(current)
            : updater;
        localStorage.setItem(SETTINGS_KEY, JSON.stringify(next));
        return next;
      });
    },
    [],
  );

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
        normalizePortProcess(item as PortProcess & { isSystemService?: boolean }),
      );
      const currentSettings = settingsRef.current;
      const prev = previousProcessesRef.current;

      if (
        !isInitialScanRef.current &&
        processesUnchanged(prev, normalized)
      ) {
        setRefreshing(false);
        setLoading(false);
        return;
      }

      if (!isInitialScanRef.current) {
        const prevByPid = new Map(prev.map((p) => [p.pid, p]));
        const nextChanges = new Map<number, RowChangeKind>();
        const toastMessages: string[] = [];

        for (const process of normalized) {
          const old = prevByPid.get(process.pid);
          if (!old) {
            nextChanges.set(process.pid, "new");
            for (const binding of process.ports) {
              toastMessages.push(
                `Port ${binding.port} is now in use by ${process.name} (PID ${process.pid})`,
              );
            }
          } else if (portSignature(old) !== portSignature(process)) {
            nextChanges.set(process.pid, "changed");
          }
          prevByPid.delete(process.pid);
        }

        for (const [, gone] of prevByPid) {
          for (const binding of gone.ports) {
            toastMessages.push(
              `Port ${binding.port} freed (${gone.name}, PID ${gone.pid})`,
            );
          }
        }

        if (nextChanges.size > 0) {
          setRowChanges(nextChanges);
          scheduleChangeClear();
        }

        if (toastMessages.length > 0 && currentSettings.showChangeToasts) {
          const preview = toastMessages.slice(0, 5);
          const remaining = toastMessages.length - preview.length;
          toast.info(
            toastMessages.length === 1
              ? "Port change detected"
              : `${toastMessages.length} port changes detected`,
            {
              description: [
                ...preview,
                ...(remaining > 0 ? [`+${remaining} more`] : []),
              ].join("\n"),
              duration: 12_000,
            },
          );
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
            void invoke("send_notification", { title, message }).catch(
              () => {
                // notifications may be unavailable
              },
            );
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
      await invoke("trigger_port_scan");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setRefreshing(false);
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    void (async () => {
      try {
        unlisten = await listen<PortsUpdatedPayload>("ports-updated", (event) => {
          const payload = parsePortsPayload(event.payload);
          applyScanResultRef.current(payload.processes, payload.error);
        });

        const payload = parsePortsPayload(
          await invoke<PortsUpdatedPayload>("get_listening_ports"),
        );

        if (payload.scanning) {
          return;
        }

        if (payload.error) {
          applyScanResultRef.current(payload.processes, payload.error);
          return;
        }

        if (payload.processes.length === 0 && !payload.error) {
          const direct = parsePortsPayload(
            await invoke<PortProcess[]>("list_listening_ports", {
              includeUdp: settingsRef.current.includeUdp,
            }),
          );
          applyScanResultRef.current(direct.processes, direct.error);
        } else {
          applyScanResultRef.current(payload.processes, payload.error);
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        setLoading(false);
        setRefreshing(false);
      }
    })();

    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    void invoke("set_scan_settings", {
      intervalMs: settings.refreshIntervalMs,
      includeUdp: settings.includeUdp,
    }).catch(() => {
      // ignore outside Tauri
    });
  }, [settings.refreshIntervalMs, settings.includeUdp]);

  useEffect(() => {
    void invoke("set_allow_system_process_actions", {
      allow: settings.allowSystemProcessActions,
    }).catch(() => {
      // ignore outside Tauri
    });
  }, [settings.allowSystemProcessActions]);

  // Keep the native tray menu's open/copy actions in sync with the URL scheme.
  useEffect(() => {
    void invoke("set_use_https_for_localhost", {
      useHttps: settings.useHttpsForLocalhost,
    }).catch(() => {
      // ignore outside Tauri
    });
  }, [settings.useHttpsForLocalhost]);

  // Keep the native tray menu's "Open in Editor" action in sync with the choice.
  useEffect(() => {
    void invoke("set_preferred_editor", {
      editor: settings.preferredEditor,
    }).catch(() => {
      // ignore outside Tauri
    });
  }, [settings.preferredEditor]);

  useEffect(() => {
    return () => {
      if (changeClearTimerRef.current !== null) {
        window.clearTimeout(changeClearTimerRef.current);
      }
    };
  }, []);

  const userCount = processes.filter((p) => !p.is_system_service).length;
  const systemCount = processes.filter((p) => p.is_system_service).length;

  useEffect(() => {
    void invoke("update_tray_count", { userCount }).catch(() => {
      // tray may be unavailable outside Tauri
    });
  }, [userCount]);

  useEffect(() => {
    void invoke("set_menu_bar_mode", { enabled: settings.menuBarMode }).catch(
      () => {
        // ignore outside Tauri
      },
    );
  }, [settings.menuBarMode]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    void listen<boolean>("tray-menu-bar-mode-changed", (event) => {
      const enabled = event.payload;
      setSettings((current) => {
        if (current.menuBarMode === enabled) {
          return current;
        }

        const next = { ...current, menuBarMode: enabled };
        localStorage.setItem(SETTINGS_KEY, JSON.stringify(next));
        return next;
      });
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
    };
  }, []);

  const setHideSystemServices = useCallback(
    (hide: boolean) => {
      persistSettings((current) => ({ ...current, hideSystemServices: hide }));
    },
    [persistSettings],
  );

  const setHideUserServices = useCallback(
    (hide: boolean) => {
      persistSettings((current) => ({ ...current, hideUserServices: hide }));
    },
    [persistSettings],
  );

  const setAllowSystemProcessActions = useCallback(
    (allow: boolean) => {
      persistSettings((current) => ({
        ...current,
        allowSystemProcessActions: allow,
      }));
    },
    [persistSettings],
  );

  const setRefreshInterval = useCallback(
    (refreshIntervalMs: RefreshInterval) => {
      persistSettings((current) => ({ ...current, refreshIntervalMs }));
    },
    [persistSettings],
  );

  const setPreferredEditor = useCallback(
    (preferredEditor: AppSettings["preferredEditor"]) => {
      persistSettings((current) => ({ ...current, preferredEditor }));
    },
    [persistSettings],
  );

  const setGroupByDirectory = useCallback(
    (groupByDirectory: boolean) => {
      persistSettings((current) => ({ ...current, groupByDirectory }));
    },
    [persistSettings],
  );

  const setShowChangeToasts = useCallback(
    (showChangeToasts: boolean) => {
      persistSettings((current) => ({ ...current, showChangeToasts }));
    },
    [persistSettings],
  );

  const setSearchField = useCallback(
    (searchField: SearchField) => {
      persistSettings((current) => ({ ...current, searchField }));
    },
    [persistSettings],
  );

  const setPinnedPaths = useCallback(
    (pinnedPaths: string[]) => {
      persistSettings((current) => ({ ...current, pinnedPaths }));
    },
    [persistSettings],
  );

  const togglePinnedPath = useCallback(
    (path: string) => {
      persistSettings((current) => {
        const pinnedPaths = current.pinnedPaths.includes(path)
          ? current.pinnedPaths.filter((item) => item !== path)
          : [...current.pinnedPaths, path];
        return { ...current, pinnedPaths };
      });
    },
    [persistSettings],
  );

  const setWatchedPorts = useCallback(
    (watchedPorts: number[]) => {
      persistSettings((current) => ({ ...current, watchedPorts }));
    },
    [persistSettings],
  );

  const setWatchedPortNotifications = useCallback(
    (watchedPortNotifications: boolean) => {
      persistSettings((current) => ({ ...current, watchedPortNotifications }));
    },
    [persistSettings],
  );

  const setIncludeUdp = useCallback(
    (includeUdp: boolean) => {
      persistSettings((current) => ({ ...current, includeUdp }));
    },
    [persistSettings],
  );

  const setUseHttpsForLocalhost = useCallback(
    (useHttpsForLocalhost: boolean) => {
      persistSettings((current) => ({ ...current, useHttpsForLocalhost }));
    },
    [persistSettings],
  );

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
    const trimmed = search.trim();
    const port = Number.parseInt(trimmed, 10);
    if (
      !Number.isInteger(port) ||
      port < 1 ||
      port > 65535 ||
      String(port) !== trimmed
    ) {
      return null;
    }
    return port;
  }, [search, settings.searchField]);

  const portLookupEmpty =
    exactPortQuery !== null &&
    !loading &&
    !processes.some((p) => processHasPort(p, exactPortQuery));

  const portLookupOccupants = useMemo(() => {
    if (exactPortQuery === null) {
      return [];
    }
    return processes.filter((process) => processHasPort(process, exactPortQuery));
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
    setSearchField,
    portLookupEmpty,
    exactPortQuery,
    portLookupOccupants,
    settings,
    setHideSystemServices,
    setHideUserServices,
    setAllowSystemProcessActions,
    setRefreshInterval,
    setPreferredEditor,
    setGroupByDirectory,
    setShowChangeToasts,
    setPinnedPaths,
    togglePinnedPath,
    setWatchedPorts,
    setWatchedPortNotifications,
    setIncludeUdp,
    setUseHttpsForLocalhost,
    setRefreshPaused,
    rowChanges,
    userCount,
    systemCount,
    hiddenSystemCount: settings.hideSystemServices ? systemCount : 0,
    hiddenUserCount: settings.hideUserServices ? userCount : 0,
  };
}
