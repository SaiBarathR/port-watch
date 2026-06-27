export type SystemKind = "apple" | "microsoft" | "distro" | "system" | "user";

export type PreferredEditor = "cursor" | "code";

export type RowChangeKind = "new" | "changed";

export interface PortBinding {
  address: string;
  port: number;
  protocol: string;
}

export interface PortProcess {
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
  uptime_seconds: number;
}

export type RefreshInterval = 3000 | 10000 | 0;

export type SearchField =
  | "all"
  | "port"
  | "pid"
  | "process"
  | "user"
  | "path"
  | "command";

export type GlassTranslucency = "subtle" | "medium" | "clear";
export type GlassBlur = "light" | "medium" | "heavy";

export const GLASS_TRANSLUCENCY_OPTIONS: {
  value: GlassTranslucency;
  label: string;
}[] = [
  { value: "subtle", label: "Subtle" },
  { value: "medium", label: "Medium" },
  { value: "clear", label: "Clear" },
];

export const GLASS_BLUR_OPTIONS: { value: GlassBlur; label: string }[] = [
  { value: "light", label: "Light" },
  { value: "medium", label: "Medium" },
  { value: "heavy", label: "Heavy" },
];

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
  menuBarMode: boolean;
  searchField: SearchField;
  pinnedPaths: string[];
  watchedPorts: number[];
  watchedPortNotifications: boolean;
  includeUdp: boolean;
  useHttpsForLocalhost: boolean;
  liquidGlass: boolean;
  glassTranslucency: GlassTranslucency;
  glassBlur: GlassBlur;
  glassTint: boolean;
}

export const DEFAULT_SETTINGS: AppSettings = {
  hideSystemServices: true,
  hideUserServices: false,
  allowSystemProcessActions: false,
  refreshIntervalMs: 3000,
  preferredEditor: "cursor",
  groupByDirectory: false,
  showChangeToasts: true,
  menuBarMode: false,
  searchField: "all",
  pinnedPaths: [],
  watchedPorts: [],
  watchedPortNotifications: false,
  includeUdp: false,
  useHttpsForLocalhost: false,
  liquidGlass: false,
  glassTranslucency: "medium",
  glassBlur: "medium",
  glassTint: true,
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

export function isVendorSystemKind(kind: SystemKind): boolean {
  return kind === "apple" || kind === "microsoft" || kind === "distro";
}

export function primaryPath(process: PortProcess): string {
  return process.script_path || process.working_directory || process.executable_path;
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

export function processHasPort(process: PortProcess, port: number): boolean {
  return process.ports.some((binding) => binding.port === port);
}

export function portSignature(process: PortProcess): string {
  return process.ports
    .map((p) => `${p.address}:${p.port}/${p.protocol}`)
    .sort()
    .join(",");
}

export function processesOnPort(
  processes: PortProcess[],
  port: number,
): PortProcess[] {
  return processes.filter((process) => processHasPort(process, port));
}
