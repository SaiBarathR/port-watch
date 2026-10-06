import { describe, expect, it } from "vitest";
import { filterPortProcesses } from "./port-filter";
import { diffProcesses } from "./scan-diff";
import type { PortBinding, PortProcess } from "./types";

function listener(
  pid: number,
  name: string,
  ports: number[],
  overrides: Partial<PortProcess> = {},
): PortProcess {
  return {
    id: `pid-${pid}`,
    pid,
    name,
    user: "dev",
    ports: ports.map((port): PortBinding => ({
      address: "*",
      port,
      protocol: "TCP",
    })),
    executable_path: `/usr/local/bin/${name}`,
    script_path: null,
    command_line: name,
    working_directory: "/Users/dev/app",
    project_root: "/Users/dev/app",
    system_kind: "user",
    is_system_service: false,
    started_at: 1_790_000_000,
    delete_blocked: null,
    ...overrides,
  };
}

const systemService = {
  is_system_service: true,
  system_kind: "apple",
} as const;

describe("diffProcesses", () => {
  it("reports nothing when nothing changed", () => {
    const scan = [listener(1, "node", [3000])];
    const { rowChanges, messages } = diffProcesses(scan, scan);
    expect(rowChanges.size).toBe(0);
    expect(messages).toEqual([]);
  });

  it("announces a new process, one line per port", () => {
    const { rowChanges, messages } = diffProcesses(
      [],
      [listener(7, "node", [3000, 3001])],
    );
    expect([...rowChanges]).toEqual([["pid-7", "new"]]);
    expect(messages).toEqual([
      "Port 3000 is now in use by node (PID 7)",
      "Port 3001 is now in use by node (PID 7)",
    ]);
  });

  it("announces the ports a vanished process freed", () => {
    const { rowChanges, messages } = diffProcesses(
      [listener(7, "node", [3000])],
      [],
    );
    expect(rowChanges.size).toBe(0);
    expect(messages).toEqual(["Port 3000 freed (node, PID 7)"]);
  });

  it("marks a surviving process whose bindings changed", () => {
    const { rowChanges } = diffProcesses(
      [listener(7, "node", [3000])],
      [listener(7, "node", [3000, 3001])],
    );
    expect([...rowChanges]).toEqual([["pid-7", "changed"]]);
  });

  it("treats a port changing hands as one process gone and one new", () => {
    const { rowChanges, messages } = diffProcesses(
      [listener(7, "node", [3000])],
      [listener(8, "node", [3000])],
    );
    expect([...rowChanges]).toEqual([["pid-8", "new"]]);
    expect(messages).toEqual([
      "Port 3000 is now in use by node (PID 8)",
      "Port 3000 freed (node, PID 7)",
    ]);
  });

  // On Linux without root, every listener whose owner is not visible has
  // PID 0. Keyed by PID they overwrote one another.
  it("tells listeners with no visible owner apart", () => {
    const ssh = listener(0, "unknown", [22], { id: "socket-tcp-0.0.0.0-22" });
    const cups = listener(0, "unknown", [631], {
      id: "socket-tcp-127.0.0.1-631",
    });
    const proxy = listener(0, "unknown", [8080], {
      id: "socket-tcp-0.0.0.0-8080",
    });

    const unchanged = diffProcesses([ssh, cups], [ssh, cups]);
    expect(unchanged.rowChanges.size).toBe(0);
    expect(unchanged.messages).toEqual([]);

    const { rowChanges, messages } = diffProcesses([ssh, cups], [ssh, proxy]);
    expect([...rowChanges]).toEqual([["socket-tcp-0.0.0.0-8080", "new"]]);
    expect(messages).toEqual([
      "Port 8080 is now in use by unknown (PID 0)",
      "Port 631 freed (unknown, PID 0)",
    ]);
  });

  // The hook diffs only what the table shows.
  it("says nothing about system services while they are hidden", () => {
    const prev = [listener(1, "node", [3000])];
    const next = [
      listener(1, "node", [3000]),
      listener(2, "rapportd", [49152], systemService),
    ];
    const inView = (processes: PortProcess[]) =>
      filterPortProcesses(processes, true, false, "", "all");

    const hidden = diffProcesses(inView(prev), inView(next));
    expect(hidden.rowChanges.size).toBe(0);
    expect(hidden.messages).toEqual([]);

    const shown = diffProcesses(prev, next);
    expect(shown.messages).toEqual([
      "Port 49152 is now in use by rapportd (PID 2)",
    ]);
  });
});
