import { describe, expect, it } from "vitest";
import { diffScans, shareUnchanged } from "@/lib/scan-diff";
import type { PortBinding, PortProcess } from "@/lib/types";

const TIMESTAMP = "2026-10-06T10:00:00.000Z";

function binding(port: number, protocol = "TCP", address = "*"): PortBinding {
  return { address, port, protocol };
}

function process(
  pid: number,
  name: string,
  ports: PortBinding[],
  overrides: Partial<PortProcess> = {},
): PortProcess {
  return {
    id: `pid-${pid}`,
    pid,
    name,
    user: "dev",
    ports,
    executable_path: "",
    script_path: null,
    command_line: "",
    working_directory: "",
    project_root: "",
    system_kind: "user",
    is_system_service: false,
    started_at: 1_790_000_000,
    delete_blocked: null,
    ...overrides,
  };
}

function diff(
  prev: PortProcess[],
  next: PortProcess[],
  options: {
    isShown?: (process: PortProcess) => boolean;
    watchedPorts?: number[];
  } = {},
) {
  return diffScans(prev, next, {
    isShown: options.isShown ?? (() => true),
    watchedPorts: options.watchedPorts ?? [],
    timestamp: TIMESTAMP,
  });
}

const summary = (events: { kind: string; port: number; pid: number }[]) =>
  events.map((event) => `${event.kind} ${event.port} by ${event.pid}`);

describe("diffScans", () => {
  it("finds nothing between two identical scans", () => {
    const scan = [process(1, "node", [binding(3000)])];
    expect(diff(scan, scan)).toEqual({
      rowChanges: new Map(),
      messages: [],
      historyEvents: [],
      watchedEvents: [],
    });
  });

  it("announces a new process once per port", () => {
    const result = diff(
      [],
      [
        process(1, "node", [
          binding(3000, "TCP", "127.0.0.1"),
          binding(3000, "TCP", "[::1]"),
          binding(9229),
        ]),
      ],
    );

    expect(result.rowChanges).toEqual(new Map([["pid-1", "new"]]));
    expect(result.messages).toEqual([
      "Port 3000 is now in use by node (PID 1)",
      "Port 9229 is now in use by node (PID 1)",
    ]);
    expect(summary(result.historyEvents)).toEqual([
      "occupied 3000 by 1",
      "occupied 9229 by 1",
    ]);
    expect(result.historyEvents[0]).toEqual({
      timestamp: TIMESTAMP,
      kind: "occupied",
      port: 3000,
      protocol: "TCP",
      pid: 1,
      processName: "node",
    });
  });

  it("announces a process that is gone", () => {
    const result = diff([process(1, "node", [binding(3000)])], []);

    expect(result.rowChanges.size).toBe(0);
    expect(result.messages).toEqual(["Port 3000 freed (node, PID 1)"]);
    expect(summary(result.historyEvents)).toEqual(["freed 3000 by 1"]);
  });

  // The toast used to stay silent here while the history recorded it.
  it("announces a port a surviving process gains or drops", () => {
    const result = diff(
      [process(1, "node", [binding(3000), binding(3001)])],
      [process(1, "node", [binding(3000), binding(3002)])],
    );

    expect(result.rowChanges).toEqual(new Map([["pid-1", "changed"]]));
    expect(result.messages).toEqual([
      "Port 3002 is now in use by node (PID 1)",
      "Port 3001 freed (node, PID 1)",
    ]);
    expect(summary(result.historyEvents)).toEqual([
      "occupied 3002 by 1",
      "freed 3001 by 1",
    ]);
  });

  it("marks a row whose address changed without calling the port new", () => {
    const result = diff(
      [process(1, "node", [binding(3000, "TCP", "127.0.0.1")])],
      [process(1, "node", [binding(3000, "TCP", "*")])],
    );

    expect(result.rowChanges).toEqual(new Map([["pid-1", "changed"]]));
    expect(result.messages).toEqual([]);
    expect(result.historyEvents).toEqual([]);
  });

  it("reports a port that passes from one process to another", () => {
    const result = diff(
      [process(1, "node", [binding(3000)])],
      [process(2, "vite", [binding(3000)])],
      { watchedPorts: [3000] },
    );

    expect(result.rowChanges).toEqual(new Map([["pid-2", "new"]]));
    expect(summary(result.historyEvents)).toEqual([
      "occupied 3000 by 2",
      "freed 3000 by 1",
    ]);
    // One alert, for the new holder, not a "free" followed by an "in use".
    expect(summary(result.watchedEvents)).toEqual(["occupied 3000 by 2"]);
  });

  it("keeps TCP and UDP on one port apart in the history", () => {
    const result = diff(
      [process(1, "dns", [binding(53, "TCP"), binding(53, "UDP")])],
      [process(1, "dns", [binding(53, "TCP")])],
    );

    expect(result.historyEvents).toEqual([
      {
        timestamp: TIMESTAMP,
        kind: "freed",
        port: 53,
        protocol: "UDP",
        pid: 1,
        processName: "dns",
      },
    ]);

    // Both taken at once read the same, and are said once.
    const both = diff(
      [],
      [process(1, "dns", [binding(53, "TCP"), binding(53, "UDP")])],
    );
    expect(both.messages).toEqual(["Port 53 is now in use by dns (PID 1)"]);
    expect(both.historyEvents).toHaveLength(2);
  });

  // On Linux every listener whose owner is not visible reports PID 0.
  it("tells two ownerless listeners apart", () => {
    const ssh = process(0, "unknown", [binding(22, "TCP", "0.0.0.0")], {
      id: "socket-tcp-0.0.0.0-22",
    });
    const https = process(0, "unknown", [binding(443, "TCP", "0.0.0.0")], {
      id: "socket-tcp-0.0.0.0-443",
    });
    const result = diff([ssh, https], [ssh]);

    expect(result.rowChanges.size).toBe(0);
    expect(summary(result.historyEvents)).toEqual(["freed 443 by 0"]);
  });

  it("keeps hidden processes out of the highlights and the toast only", () => {
    const system = process(9, "rapportd", [binding(49152)], {
      is_system_service: true,
    });
    const result = diff([], [system, process(1, "node", [binding(3000)])], {
      isShown: (candidate) => !candidate.is_system_service,
      watchedPorts: [49152],
    });

    expect(result.rowChanges).toEqual(new Map([["pid-1", "new"]]));
    expect(result.messages).toEqual([
      "Port 3000 is now in use by node (PID 1)",
    ]);
    expect(summary(result.historyEvents)).toEqual([
      "occupied 49152 by 9",
      "occupied 3000 by 1",
    ]);
    expect(summary(result.watchedEvents)).toEqual(["occupied 49152 by 9"]);
  });

  describe("watched ports", () => {
    it("alerts when a watched port is taken and when it is freed", () => {
      const node = process(1, "node", [binding(3000), binding(4000)]);

      expect(
        summary(diff([], [node], { watchedPorts: [3000] }).watchedEvents),
      ).toEqual(["occupied 3000 by 1"]);
      expect(
        summary(diff([node], [], { watchedPorts: [3000] }).watchedEvents),
      ).toEqual(["freed 3000 by 1"]);
      expect(diff([], [node], { watchedPorts: [5000] }).watchedEvents).toEqual(
        [],
      );
    });

    // A master and its workers share the socket. One owner per port used to
    // be remembered, so a worker restart looked like the port changing hands.
    it("stays quiet while any holder of a shared port remains", () => {
      const master = process(100, "nginx", [binding(80)]);
      const worker = process(101, "nginx", [binding(80)]);
      const newWorker = process(102, "nginx", [binding(80)]);

      const result = diff([master, worker], [master, newWorker], {
        watchedPorts: [80],
      });

      expect(result.watchedEvents).toEqual([]);
      // The history still has the worker coming and going.
      expect(summary(result.historyEvents)).toEqual([
        "occupied 80 by 102",
        "freed 80 by 101",
      ]);
    });

    it("alerts once for a port listed twice", () => {
      const result = diff([], [process(1, "node", [binding(3000)])], {
        watchedPorts: [3000, 3000],
      });
      expect(result.watchedEvents).toHaveLength(1);
    });
  });
});

describe("shareUnchanged", () => {
  it("hands back the previous list when nothing changed", () => {
    const prev = [
      process(1, "node", [binding(3000)]),
      process(2, "vite", [binding(5173)]),
    ];
    const next = structuredClone(prev);

    expect(shareUnchanged(prev, next)).toBe(prev);
  });

  it("keeps the objects of processes that did not change", () => {
    const prev = [
      process(1, "node", [binding(3000)]),
      process(2, "vite", [binding(5173)]),
    ];
    const next = [
      structuredClone(prev[0]),
      process(2, "vite", [binding(5173), binding(5174)]),
      process(3, "bun", [binding(8080)]),
    ];

    const shared = shareUnchanged(prev, next);

    expect(shared).not.toBe(prev);
    expect(shared[0]).toBe(prev[0]);
    expect(shared[1]).toBe(next[1]);
    expect(shared[2]).toBe(next[2]);
  });

  it("is a new list when only the order changed", () => {
    const prev = [
      process(1, "node", [binding(3000)]),
      process(2, "vite", [binding(5173)]),
    ];
    const next = [structuredClone(prev[1]), structuredClone(prev[0])];

    const shared = shareUnchanged(prev, next);

    expect(shared).not.toBe(prev);
    expect(shared).toEqual(next);
    expect(shared[0]).toBe(prev[1]);
  });

  it("is a new list when a process went away", () => {
    const prev = [
      process(1, "node", [binding(3000)]),
      process(2, "vite", [binding(5173)]),
    ];
    const shared = shareUnchanged(prev, [structuredClone(prev[0])]);

    expect(shared).not.toBe(prev);
    expect(shared).toEqual([prev[0]]);
  });
});
