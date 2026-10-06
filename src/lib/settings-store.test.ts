import { describe, expect, it } from "vitest";
import { readSettings } from "@/lib/settings-store";
import { DEFAULT_SETTINGS } from "@/lib/types";

describe("readSettings", () => {
  it("gives the defaults for anything that is not settings", () => {
    for (const raw of [null, undefined, "x", 42, [], {}]) {
      expect(readSettings(raw)).toEqual(DEFAULT_SETTINGS);
    }
  });

  it("keeps every valid value", () => {
    const stored = {
      hideSystemServices: false,
      hideUserServices: true,
      allowSystemProcessActions: true,
      refreshIntervalMs: 10000,
      preferredEditor: "code",
      groupByDirectory: true,
      showChangeToasts: false,
      changeToastsMutedUntil: 1_791_000_000_000,
      menuBarMode: true,
      searchField: "port",
      pinnedPaths: ["/Users/dev/app"],
      watchedPorts: [3000, 8080],
      watchedPortNotifications: true,
      includeUdp: true,
      useHttpsForLocalhost: true,
    };
    expect(readSettings(stored)).toEqual(stored);
  });

  it("replaces only the values it cannot use", () => {
    const settings = readSettings({
      refreshIntervalMs: 1234,
      preferredEditor: "vim",
      searchField: "everything",
      includeUdp: "yes",
      changeToastsMutedUntil: "tomorrow",
      groupByDirectory: true,
      menuBarMode: true,
    });

    expect(settings).toEqual({
      ...DEFAULT_SETTINGS,
      groupByDirectory: true,
      menuBarMode: true,
    });
  });

  it("drops watched ports and pinned paths that are not ports or paths", () => {
    const settings = readSettings({
      watchedPorts: [3000, 3000.5, "8080", 0, 65536, -1, 65535, null],
      pinnedPaths: ["/a", 7, null, "/b"],
    });

    expect(settings.watchedPorts).toEqual([3000, 65535]);
    expect(settings.pinnedPaths).toEqual(["/a", "/b"]);
  });

  it("never hides both user and system processes", () => {
    const settings = readSettings({
      hideSystemServices: true,
      hideUserServices: true,
    });

    expect(settings.hideSystemServices).toBe(true);
    expect(settings.hideUserServices).toBe(false);
  });

  it("ignores settings it does not know", () => {
    expect(readSettings({ somethingNew: 1 })).toEqual(DEFAULT_SETTINGS);
  });
});
