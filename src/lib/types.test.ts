import { describe, expect, it } from "vitest";
import {
  formatUptime,
  oldestFirst,
  parsePort,
  uptimeSeconds,
  userProcesses,
  type PortProcess,
} from "./types";

function sampleProcess(overrides: Partial<PortProcess> = {}): PortProcess {
  return {
    id: `pid-${overrides.pid ?? 1}`,
    pid: 1,
    name: "node",
    user: "dev",
    ports: [{ address: "*", port: 3000, protocol: "TCP" }],
    executable_path: "/usr/local/bin/node",
    script_path: null,
    command_line: "node server.js",
    working_directory: "/Users/dev/app",
    project_root: "/Users/dev/app",
    system_kind: "user",
    is_system_service: false,
    started_at: 1_790_000_000,
    delete_blocked: null,
    ...overrides,
  };
}

describe("userProcesses", () => {
  // "Stop all visible user processes" used to take every row the user was
  // allowed to stop, which includes system services once that is opted into.
  it("never includes a system service", () => {
    const processes = [
      sampleProcess({ pid: 1 }),
      sampleProcess({ pid: 2, is_system_service: true, system_kind: "apple" }),
      sampleProcess({ pid: 3, is_system_service: true, system_kind: "system" }),
      sampleProcess({ pid: 4 }),
    ];

    expect(userProcesses(processes).map((process) => process.pid)).toEqual([
      1, 4,
    ]);
  });

  it("returns nothing when only system services are visible", () => {
    const processes = [
      sampleProcess({ pid: 2, is_system_service: true, system_kind: "apple" }),
    ];

    expect(userProcesses(processes)).toEqual([]);
  });
});

describe("parsePort", () => {
  it("reads a port number", () => {
    expect(parsePort("3000")).toBe(3000);
    expect(parsePort("  8080 ")).toBe(8080);
    expect(parsePort("1")).toBe(1);
    expect(parsePort("65535")).toBe(65535);
  });

  it("refuses anything that is not only a port number", () => {
    for (const text of [
      "",
      "3000abc",
      "3000.5",
      "-1",
      "0",
      "65536",
      "999999",
      "1e3",
      "0x50",
      "30 00",
    ]) {
      expect(parsePort(text), text).toBeNull();
    }
  });
});

describe("uptimeSeconds", () => {
  it("is the time since the process started", () => {
    expect(uptimeSeconds(1_000, 1_090)).toBe(90);
  });

  it("is zero, shown as a dash, when the start time is unknown", () => {
    expect(uptimeSeconds(0, 1_090)).toBe(0);
    expect(formatUptime(uptimeSeconds(0, 1_090))).toBe("—");
  });

  it("never goes negative when the clocks disagree", () => {
    expect(uptimeSeconds(1_100, 1_090)).toBe(0);
  });
});

describe("oldestFirst", () => {
  it("puts the longest-running process first and unknown start times last", () => {
    const processes = [
      sampleProcess({ pid: 1, started_at: 300 }),
      sampleProcess({ pid: 2, started_at: 0 }),
      sampleProcess({ pid: 3, started_at: 100 }),
      sampleProcess({ pid: 4, started_at: 200 }),
    ];

    expect(oldestFirst(processes).map((process) => process.pid)).toEqual([
      3, 4, 1, 2,
    ]);
    expect(processes.map((process) => process.pid)).toEqual([1, 2, 3, 4]);
  });
});
