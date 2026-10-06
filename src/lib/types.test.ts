import { describe, expect, it } from "vitest";
import { userProcesses, type PortProcess } from "./types";

function sampleProcess(overrides: Partial<PortProcess> = {}): PortProcess {
  return {
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
    uptime_seconds: 10,
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
