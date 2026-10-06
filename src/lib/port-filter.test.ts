import { describe, expect, it } from "vitest";
import { filterPortProcesses } from "./port-filter";
import type { PortProcess } from "./types";

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

describe("filterPortProcesses", () => {
  it("respects hideUserServices without fallback", () => {
    const processes = [sampleProcess()];
    const filtered = filterPortProcesses(processes, false, true, "", "all");
    expect(filtered).toEqual([]);
  });

  it("falls back to user processes when system services are hidden", () => {
    const processes = [
      sampleProcess({ pid: 1, is_system_service: true, system_kind: "system" }),
      sampleProcess({ pid: 2, is_system_service: false, system_kind: "user" }),
    ];
    const filtered = filterPortProcesses(processes, true, false, "", "all");
    expect(filtered.map((process) => process.pid)).toEqual([2]);
  });

  it("searches listeners that share PID 0 separately", () => {
    const processes = [
      sampleProcess({
        id: "socket-tcp-0.0.0.0-22",
        pid: 0,
        name: "unknown",
        ports: [{ address: "0.0.0.0", port: 22, protocol: "TCP" }],
      }),
      sampleProcess({
        id: "socket-tcp-0.0.0.0-631",
        pid: 0,
        name: "unknown",
        ports: [{ address: "0.0.0.0", port: 631, protocol: "TCP" }],
      }),
    ];

    const filtered = filterPortProcesses(processes, false, false, "631", "all");

    expect(filtered.map((process) => process.id)).toEqual([
      "socket-tcp-0.0.0.0-631",
    ]);
  });
});
