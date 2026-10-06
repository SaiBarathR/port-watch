export type SystemKind = "apple" | "microsoft" | "distro" | "system" | "user";

export type PreferredEditor = "cursor" | "code";

export type RowChangeKind = "new" | "changed";

export interface PortBinding {
  address: string;
  port: number;
  protocol: string;
}

export interface PortProcess {
  /**
   * Names this row across scans. The PID cannot: on Linux every listener
   * whose owner is not visible is reported under PID 0.
   */
  id: string;
  pid: number;
  name: string;
  user: string;
  ports: PortBinding[];
  executable_path: string;
  script_path: string | null;
  command_line: string;
  working_directory: string;
  project_root: string;
  system_kind: SystemKind;
  is_system_service: boolean;
  /** When the process started, in Unix seconds; 0 when that is unknown. */
  started_at: number;
  /** Why the project folder cannot be deleted from the app, if it cannot. */
  delete_blocked: string | null;
}

export type RefreshInterval = 3000 | 10000 | 0;

export type SearchField =
  "all" | "port" | "pid" | "process" | "user" | "path" | "command";

export const SEARCH_FIELD_OPTIONS: { value: SearchField; label: string }[] = [
  { value: "all", label: "All fields" },
  { value: "port", label: "Port" },
  { value: "pid", label: "PID" },
  { value: "process", label: "Process" },
  { value: "user", label: "User" },
  { value: "path", label: "Path" },
  { value: "command", label: "Command" },
];

export interface AppSettings {
  hideSystemServices: boolean;
  hideUserServices: boolean;
  allowSystemProcessActions: boolean;
  refreshIntervalMs: RefreshInterval;
  preferredEditor: PreferredEditor;
  groupByDirectory: boolean;
  showChangeToasts: boolean;
  changeToastsMutedUntil: number | null;
  menuBarMode: boolean;
  searchField: SearchField;
  pinnedPaths: string[];
  watchedPorts: number[];
  watchedPortNotifications: boolean;
  includeUdp: boolean;
  useHttpsForLocalhost: boolean;
}

export const DEFAULT_SETTINGS: AppSettings = {
  hideSystemServices: true,
  hideUserServices: false,
  allowSystemProcessActions: false,
  refreshIntervalMs: 3000,
  preferredEditor: "cursor",
  groupByDirectory: false,
  showChangeToasts: true,
  changeToastsMutedUntil: null,
  menuBarMode: false,
  searchField: "all",
  pinnedPaths: [],
  watchedPorts: [],
  watchedPortNotifications: false,
  includeUdp: false,
  useHttpsForLocalhost: false,
};

export function formatPorts(
  ports: PortBinding[],
  includeProtocol = false,
): string {
  return ports
    .map((p) => {
      let label: string;
      if (p.address === "*" || p.address === "0.0.0.0") {
        label = String(p.port);
      } else {
        label = `${p.address}:${p.port}`;
      }
      if (includeProtocol) {
        return `${label}/${p.protocol.toLowerCase()}`;
      }
      return label;
    })
    .join(", ");
}

/** Seconds a process has been running; 0 when its start time is unknown. */
export function uptimeSeconds(startedAt: number, nowSeconds: number): number {
  return startedAt > 0 ? Math.max(0, nowSeconds - startedAt) : 0;
}

export function formatUptime(seconds: number): string {
  if (seconds <= 0) {
    return "—";
  }
  const days = Math.floor(seconds / 86_400);
  const hours = Math.floor((seconds % 86_400) / 3_600);
  const minutes = Math.floor((seconds % 3_600) / 60);
  const secs = seconds % 60;

  if (days > 0) {
    return `${days}d ${hours}h`;
  }
  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  if (minutes > 0) {
    return `${minutes}m ${secs}s`;
  }
  return `${secs}s`;
}

export function systemKindLabel(kind: SystemKind): string {
  switch (kind) {
    case "apple":
      return "Apple System";
    case "microsoft":
      return "Microsoft System";
    case "distro":
      return "Distro System";
    case "system":
      return "System";
    case "user":
      return "User";
  }
}

export function primaryPath(process: PortProcess): string {
  return (
    process.script_path || process.working_directory || process.executable_path
  );
}

export function groupDirectory(process: PortProcess): string {
  return (
    process.project_root ||
    process.working_directory ||
    primaryPath(process) ||
    "Unknown"
  );
}

export function pinPath(process: PortProcess): string {
  return process.project_root || process.working_directory || "";
}

export function isPinned(process: PortProcess, pinnedPaths: string[]): boolean {
  const path = pinPath(process);
  return path !== "" && pinnedPaths.includes(path);
}

export function localhostUrl(port: number, useHttps = false): string {
  return `${useHttps ? "https" : "http"}://localhost:${port}`;
}

export function primaryPort(process: PortProcess): number | null {
  return process.ports[0]?.port ?? null;
}

/**
 * A port number typed by the user, or null. Digits only: "3000abc" and
 * "3000.5" are not ports, though parseInt would read 3000 from either.
 */
export function parsePort(text: string): number | null {
  const trimmed = text.trim();
  if (!/^\d{1,5}$/.test(trimmed)) {
    return null;
  }
  const port = Number(trimmed);
  return port >= 1 && port <= 65535 ? port : null;
}

export function processHasPort(process: PortProcess, port: number): boolean {
  return process.ports.some((binding) => binding.port === port);
}

export function portSignature(process: PortProcess): string {
  return process.ports
    .map((p) => `${p.address}:${p.port}/${p.protocol}`)
    .sort()
    .join(",");
}

/**
 * Oldest first, a process with an unknown start time last. A parent predates
 * its children, and stopping it first keeps it from respawning a worker that
 * was stopped a moment earlier.
 */
export function oldestFirst(processes: PortProcess[]): PortProcess[] {
  const startedAt = (process: PortProcess) =>
    process.started_at > 0 ? process.started_at : Number.POSITIVE_INFINITY;
  return [...processes].sort((a, b) => startedAt(a) - startedAt(b));
}

/**
 * Targets of "Stop all visible user processes". Never a system service, even
 * when system process actions are allowed: those are stopped one at a time,
 * behind their own confirmation.
 */
export function userProcesses(processes: PortProcess[]): PortProcess[] {
  return processes.filter((process) => !process.is_system_service);
}

export function processesOnPort(
  processes: PortProcess[],
  port: number,
): PortProcess[] {
  return processes.filter((process) => processHasPort(process, port));
}
