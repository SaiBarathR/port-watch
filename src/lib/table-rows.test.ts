import { describe, expect, it } from "vitest";
import { sortForTable, withGroupHeaders } from "./table-rows";
import type { PortProcess } from "./types";

function listener(port: number, directory: string): PortProcess {
  return {
    pid: port,
    name: "node",
    user: "dev",
    ports: [{ address: "*", port, protocol: "TCP" }],
    executable_path: "/usr/local/bin/node",
    script_path: null,
    command_line: "node server.js",
    working_directory: directory,
    project_root: directory,
    system_kind: "user",
    is_system_service: false,
    started_at: 1_790_000_000,
    delete_blocked: null,
  };
}

// One line per table row: a header as "[label]", a listener as its port.
function layout(
  processes: PortProcess[],
  order: { pinnedPaths: string[]; groupByDirectory: boolean },
): string[] {
  return withGroupHeaders(
    sortForTable(processes, order),
    (process) => process,
    order,
  ).map((item) =>
    item.kind === "group" ? `[${item.label}]` : String(item.row.ports[0].port),
  );
}

const processes = [
  listener(8080, "/proj/web"),
  listener(3000, "/proj/api"),
  listener(5173, "/proj/web"),
  listener(9000, "/proj/tools"),
];

describe("table rows", () => {
  it("orders by port and adds no headers when nothing is pinned or grouped", () => {
    expect(
      layout(processes, { pinnedPaths: [], groupByDirectory: false }),
    ).toEqual(["3000", "5173", "8080", "9000"]);
  });

  it("separates pinned rows from the rest", () => {
    expect(
      layout(processes, {
        pinnedPaths: ["/proj/web"],
        groupByDirectory: false,
      }),
    ).toEqual([
      "[Pinned]",
      "5173",
      "8080",
      "[Other listeners]",
      "3000",
      "9000",
    ]);
  });

  it("adds only the Pinned header when every row is pinned", () => {
    expect(
      layout(processes, {
        pinnedPaths: ["/proj/web", "/proj/api", "/proj/tools"],
        groupByDirectory: false,
      }),
    ).toEqual(["[Pinned]", "3000", "5173", "8080", "9000"]);
  });

  it("groups by directory", () => {
    expect(
      layout(processes, { pinnedPaths: [], groupByDirectory: true }),
    ).toEqual([
      "[/proj/api]",
      "3000",
      "[/proj/tools]",
      "9000",
      "[/proj/web]",
      "5173",
      "8080",
    ]);
  });

  it("keeps unpinned directories out of the Pinned section when grouping", () => {
    expect(
      layout(processes, { pinnedPaths: ["/proj/web"], groupByDirectory: true }),
    ).toEqual([
      "[Pinned]",
      "[/proj/web]",
      "5173",
      "8080",
      "[Other listeners]",
      "[/proj/api]",
      "3000",
      "[/proj/tools]",
      "9000",
    ]);
  });

  it("gives every header a distinct id", () => {
    const items = withGroupHeaders(
      sortForTable(processes, {
        pinnedPaths: ["/proj/web"],
        groupByDirectory: true,
      }),
      (process) => process,
      { pinnedPaths: ["/proj/web"], groupByDirectory: true },
    );
    const ids = items.flatMap((item) =>
      item.kind === "group" ? [item.id] : [],
    );
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("returns nothing for an empty table", () => {
    expect(
      layout([], { pinnedPaths: ["/proj/web"], groupByDirectory: true }),
    ).toEqual([]);
  });
});
