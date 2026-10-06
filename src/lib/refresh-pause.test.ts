import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() =>
  vi.fn((command: string, args: { paused: boolean }) =>
    Promise.resolve(`${command}:${args.paused}`),
  ),
);
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { holdRefresh } from "@/lib/refresh-pause";

// The backend is told at the end of the turn.
const sent = async () => {
  await Promise.resolve();
  return invoke.mock.calls.map(
    ([command, args]) => `${command}:${args.paused}`,
  );
};

describe("holdRefresh", () => {
  beforeEach(() => {
    invoke.mockClear();
  });

  it("pauses on the first reason and resumes when the last lets go", async () => {
    const releaseMenu = holdRefresh("menu");
    const releaseDialog = holdRefresh("dialog");
    expect(await sent()).toEqual(["set_refresh_paused:true"]);

    // The dialog closes while the menu is still open: scans stay paused.
    releaseDialog();
    expect(await sent()).toEqual(["set_refresh_paused:true"]);

    releaseMenu();
    expect(await sent()).toEqual([
      "set_refresh_paused:true",
      "set_refresh_paused:false",
    ]);
  });

  // A row menu closes as the dialog it opened appears. The backend scans the
  // moment it hears "resume", so it must not hear it in between.
  it("says nothing when one reason gives way to another in the same turn", async () => {
    const releaseMenu = holdRefresh("menu");
    expect(await sent()).toEqual(["set_refresh_paused:true"]);

    releaseMenu();
    const releaseDialog = holdRefresh("dialog");
    expect(await sent()).toEqual(["set_refresh_paused:true"]);

    releaseDialog();
    expect(await sent()).toEqual([
      "set_refresh_paused:true",
      "set_refresh_paused:false",
    ]);
  });

  it("ignores a release that comes twice", async () => {
    const release = holdRefresh("menu");
    const other = holdRefresh("dialog");
    release();
    release();
    expect(await sent()).toEqual(["set_refresh_paused:true"]);

    other();
    expect(await sent()).toEqual([
      "set_refresh_paused:true",
      "set_refresh_paused:false",
    ]);
  });
});
