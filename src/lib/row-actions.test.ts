import { describe, expect, it } from "vitest";
import {
  rowActions,
  stopBlockedReason,
  type RowActionContext,
} from "@/lib/row-actions";
import type { PortProcess } from "@/lib/types";

function process(overrides: Partial<PortProcess> = {}): PortProcess {
  return {
    id: "pid-1",
    pid: 1,
    name: "node",
    user: "dev",
    ports: [{ address: "*", port: 3000, protocol: "TCP" }],
    executable_path: "/usr/bin/node",
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

function context(overrides: Partial<RowActionContext> = {}): RowActionContext {
  return {
    platform: "macos",
    preferredEditor: "cursor",
    pinnedPaths: [],
    watchedPorts: [],
    allowSystemProcessActions: false,
    portIsShared: false,
    alternate: false,
    ...overrides,
  };
}

const labels = (groups: ReturnType<typeof rowActions>) =>
  groups.map((group) => group.map((action) => action.label));
const find = (groups: ReturnType<typeof rowActions>, id: string) =>
  groups.flat().find((action) => action.id === id);

describe("rowActions", () => {
  it("goes from opening to deleting, with the destructive items last", () => {
    expect(labels(rowActions(process(), context()))).toEqual([
      ["Open in Browser", "Copy URL"],
      [
        "Open in Cursor",
        "Open in Terminal",
        "Reveal in Finder",
        "Copy Folder Path",
      ],
      ["Pin Project", "Watch Port 3000", "Port 3000 History"],
      ["Stop…"],
      ["Move to Trash…"],
    ]);
  });

  it("names things the way each platform does", () => {
    const windows = rowActions(
      process(),
      context({ platform: "windows", preferredEditor: "code" }),
    );
    expect(find(windows, "reveal")?.label).toBe("Show in Explorer");
    expect(find(windows, "trash")?.label).toBe("Move to Recycle Bin…");
    expect(find(windows, "open-editor")).toMatchObject({
      label: "Open in VS Code",
      shortcut: "Ctrl+O",
    });
    expect(find(rowActions(process(), context()), "stop")?.shortcut).toBe("⌘⌫");
  });

  it("reflects what is already pinned and watched", () => {
    const groups = rowActions(
      process(),
      context({ pinnedPaths: ["/Users/dev/app"], watchedPorts: [3000] }),
    );
    expect(find(groups, "pin")?.label).toBe("Unpin Project");
    expect(find(groups, "watch")?.label).toBe("Stop Watching Port 3000");
  });

  it("offers to free the port only when others hold it too", () => {
    expect(find(rowActions(process(), context()), "free-port")).toBeUndefined();
    expect(
      find(rowActions(process(), context({ portIsShared: true })), "free-port")
        ?.label,
    ).toBe("Free Port 3000…");
  });

  it("shows the permanent delete in place of the trash while the key is held", () => {
    const groups = rowActions(process(), context({ alternate: true }));
    expect(find(groups, "trash")).toBeUndefined();
    expect(find(groups, "delete")).toMatchObject({
      label: "Delete Permanently…",
      destructive: true,
    });
  });

  it("says why a locked system service cannot be stopped or deleted", () => {
    const system = process({ is_system_service: true, system_kind: "apple" });
    const locked = rowActions(system, context());
    expect(find(locked, "stop")?.disabledReason).toMatch(/locked/);
    expect(find(locked, "trash")?.disabledReason).toMatch(/locked/);
    // Opening and tracking are still allowed.
    expect(find(locked, "open-browser")?.disabledReason).toBeUndefined();
    expect(find(locked, "watch")?.disabledReason).toBeUndefined();

    const allowed = rowActions(
      system,
      context({ allowSystemProcessActions: true }),
    );
    expect(find(allowed, "stop")?.disabledReason).toBeUndefined();
  });

  it("says why a listener with no visible owner cannot be stopped", () => {
    const ownerless = process({ pid: 0, id: "socket-tcp-0.0.0.0-3000" });
    expect(
      find(rowActions(ownerless, context()), "stop")?.disabledReason,
    ).toMatch(/not visible/);
    expect(stopBlockedReason(ownerless, true)).toMatch(/not visible/);
  });

  it("disables what needs a folder when none is known", () => {
    const groups = rowActions(
      process({ project_root: "", working_directory: "" }),
      context(),
    );
    for (const id of [
      "open-editor",
      "open-terminal",
      "reveal",
      "copy-path",
      "pin",
      "trash",
    ]) {
      expect(find(groups, id)?.disabledReason, id).toMatch(/No folder/);
    }
    expect(find(groups, "stop")?.disabledReason).toBeUndefined();
  });

  it("passes on the backend's reason a folder cannot be deleted", () => {
    const groups = rowActions(
      process({ delete_blocked: "This folder is outside your home folder." }),
      context(),
    );
    expect(find(groups, "trash")?.disabledReason).toBe(
      "This folder is outside your home folder.",
    );
  });
});
