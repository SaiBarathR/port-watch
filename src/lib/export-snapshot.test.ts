import { describe, expect, it } from "vitest";
import { processesToJson, processesToMarkdown } from "@/lib/export-snapshot";
import type { PortProcess } from "@/lib/types";

function listener(overrides: Partial<PortProcess> = {}): PortProcess {
  return {
    id: "pid-4242",
    pid: 4242,
    name: "node",
    user: "dev",
    ports: [{ address: "127.0.0.1", port: 3000, protocol: "TCP" }],
    executable_path: "/usr/local/bin/node",
    script_path: null,
    command_line: "node server.js",
    working_directory: "/Users/dev/app/src",
    project_root: "/Users/dev/app",
    system_kind: "user",
    is_system_service: false,
    started_at: 1_790_000_000,
    delete_blocked: null,
    ...overrides,
  };
}

describe("export as JSON", () => {
  it("is the rows as the backend reported them", () => {
    const rows = [listener(), listener({ id: "pid-7", pid: 7 })];

    expect(JSON.parse(processesToJson(rows))).toEqual(rows);
  });
});

describe("export as Markdown", () => {
  it("gives each row its ports in full, with address and protocol", () => {
    const table = processesToMarkdown([
      listener({
        ports: [
          { address: "127.0.0.1", port: 3000, protocol: "TCP" },
          { address: "::1", port: 3000, protocol: "TCP" },
          { address: "*", port: 5353, protocol: "UDP" },
        ],
      }),
    ]);

    expect(table.split("\n")).toEqual([
      "| Port(s) | Process | PID | User | Type | Directory |",
      "| --- | --- | --- | --- | --- | --- |",
      "| 127.0.0.1:3000/tcp, ::1:3000/tcp, *:5353/udp | node | 4242 | dev | User | /Users/dev/app |",
    ]);
  });

  it("names the folder the process runs in when no project was found", () => {
    const table = processesToMarkdown([listener({ project_root: "" })]);

    expect(table).toContain("| /Users/dev/app/src |");
  });

  it("keeps a name with a bar or a line break inside its cell", () => {
    const table = processesToMarkdown([listener({ name: "a|b\nc" })]);

    expect(table.split("\n")).toHaveLength(3);
    expect(table).toContain("| a\\|b c |");
  });

  it("says so when there is nothing to export", () => {
    expect(processesToMarkdown([])).toBe("_No processes._");
  });
});
