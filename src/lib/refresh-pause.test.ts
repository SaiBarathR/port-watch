import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() =>
  vi.fn((command: string, args: { paused: boolean }) =>
    Promise.resolve(`${command}:${args.paused}`),
  ),
);
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { holdRefresh } from "@/lib/refresh-pause";

const sent = () =>
  invoke.mock.calls.map(([command, args]) => `${command}:${args.paused}`);

describe("holdRefresh", () => {
  beforeEach(() => {
    invoke.mockClear();
  });

  it("pauses on the first reason and resumes when the last lets go", () => {
    const releaseMenu = holdRefresh("menu");
    const releaseDialog = holdRefresh("dialog");
    expect(sent()).toEqual(["set_refresh_paused:true"]);

    // The dialog closes while the menu is still open: scans stay paused.
    releaseDialog();
    expect(sent()).toEqual(["set_refresh_paused:true"]);

    releaseMenu();
    expect(sent()).toEqual([
      "set_refresh_paused:true",
      "set_refresh_paused:false",
    ]);
  });

  it("ignores a release that comes twice", () => {
    const release = holdRefresh("menu");
    const other = holdRefresh("dialog");
    release();
    release();
    expect(sent()).toEqual(["set_refresh_paused:true"]);

    other();
    expect(sent()).toEqual([
      "set_refresh_paused:true",
      "set_refresh_paused:false",
    ]);
  });
});
