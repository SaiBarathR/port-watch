import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { act, render } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { toast } from "sonner";
import { vi } from "vitest";
import {
  DEFAULT_SETTINGS,
  type AppSettings,
  type PortProcess,
} from "@/lib/types";

/** A listener as a scan reports it: a dev server unless told otherwise. */
export function listener(
  pid: number,
  port: number,
  overrides: Partial<PortProcess> = {},
): PortProcess {
  return {
    id: `pid-${pid}`,
    pid,
    name: "node",
    user: "dev",
    ports: [{ address: "127.0.0.1", port, protocol: "TCP" }],
    executable_path: "/usr/local/bin/node",
    script_path: null,
    command_line: "node server.js",
    working_directory: `/Users/dev/app-${port}`,
    project_root: `/Users/dev/app-${port}`,
    system_kind: "user",
    is_system_service: false,
    started_at: 1_790_000_000,
    delete_blocked: null,
    ...overrides,
  };
}

/** One the operating system runs, which the app must not stop by default. */
export function systemService(pid: number, port: number, name: string) {
  return listener(pid, port, {
    name,
    user: "root",
    executable_path: `/usr/sbin/${name}`,
    command_line: `/usr/sbin/${name}`,
    working_directory: "/",
    project_root: "",
    system_kind: "apple",
    is_system_service: true,
  });
}

interface Launch {
  processes?: PortProcess[];
  settings?: Partial<AppSettings>;
  /** Leaves the first scan unanswered until `finishFirstScan` is called. */
  holdFirstScan?: boolean;
  /** PIDs the backend refuses to stop, and what it says. */
  refuseToStop?: Record<number, string>;
  /** What the backend says instead of deleting a project. */
  refuseToDelete?: string;
  /** What the backend says instead of saving a settings change. */
  refuseSettings?: string;
}

interface Scan {
  /** Whether this scan looked for UDP sockets too. */
  includeUdp?: boolean;
}

/**
 * Starts the app as a launch does, against a backend that answers the
 * window's commands from memory. The app's modules are loaded afresh, so
 * nothing a previous test left in them is seen.
 */
export async function launchApp(launch: Launch = {}) {
  vi.resetModules();
  clearMocks();
  for (const item of toast.getToasts()) {
    toast.dismiss(item.id);
  }
  Object.assign(window, {
    __TAURI_OS_PLUGIN_INTERNALS__: { platform: "macos" },
  });

  let processes = launch.processes ?? [];
  let settings: AppSettings = { ...DEFAULT_SETTINGS, ...launch.settings };
  let scanRevision = 1;
  let includeUdp = settings.includeUdp;
  let settingsRevision = 1;
  let releaseFirstScan = () => {};
  const firstScanReleased = launch.holdFirstScan
    ? new Promise<void>((resolve) => {
        releaseFirstScan = resolve;
      })
    : Promise.resolve();
  const calls: { command: string; args: Record<string, unknown> }[] = [];

  const scan = () => ({
    processes,
    error: null,
    revision: scanRevision,
    include_udp: includeUdp,
  });
  const without = (pid: unknown) => {
    processes = processes.filter((process) => process.pid !== pid);
    scanRevision += 1;
  };

  mockWindows("main");
  mockIPC(
    (command, payload) => {
      const args = (payload ?? {}) as Record<string, unknown>;
      calls.push({ command, args });

      switch (command) {
        case "get_settings":
          return { settings, revision: settingsRevision, adopted: true };
        case "update_settings":
          if (launch.refuseSettings) {
            throw launch.refuseSettings;
          }
          settings = { ...settings, ...(args.patch as Partial<AppSettings>) };
          settingsRevision += 1;
          return { settings, revision: settingsRevision };
        case "get_listening_ports":
          return firstScanReleased.then(scan);
        case "trigger_port_scan":
          return scan();
        case "stop_process": {
          const refusal = launch.refuseToStop?.[args.pid as number];
          if (refusal) {
            throw refusal;
          }
          without(args.pid);
          return null;
        }
        case "delete_project":
          if (launch.refuseToDelete) {
            throw launch.refuseToDelete;
          }
          without(args.pid);
          return null;
        case "get_cli_install_status":
          return {
            installed: true,
            pointsToApp: true,
            linkPath: "/usr/local/bin/port-watch",
            targetPath: "/Applications/Port Watch.app",
          };
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );

  const { initSettings } = await import("@/lib/settings-store");
  const { default: App } = await import("@/App");
  await initSettings();
  render(<App />);

  return {
    user: userEvent.setup(),
    /** The arguments of every call the window made to one command. */
    callsTo: (command: string) =>
      calls.filter((call) => call.command === command).map((call) => call.args),
    /** The window comes to the front, as after a switch from another app. */
    focusWindow: async () => {
      await act(async () => {
        await emit("tauri://focus");
      });
    },
    finishFirstScan: async () => {
      await act(async () => releaseFirstScan());
    },
    /** A later scan finds this instead, and says so as the poller does. */
    scanFinds: async (next: PortProcess[], found: Scan = {}) => {
      processes = next;
      includeUdp = found.includeUdp ?? includeUdp;
      scanRevision += 1;
      await act(async () => {
        await emit("ports-updated", scan());
      });
    },
  };
}
