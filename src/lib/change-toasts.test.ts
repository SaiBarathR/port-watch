import { describe, expect, it } from "vitest";
import {
  MUTE_DURATIONS,
  changeToastStatus,
  isChangeToastsMuted,
  addPortChanges,
  isPortChangeToastId,
  portChangeToastContent,
} from "./change-toasts";

const NOW = Date.UTC(2026, 9, 6, 12, 0, 0);
const LONGEST_MUTE_MS = Math.max(
  ...MUTE_DURATIONS.map((duration) => duration.ms),
);

describe("isChangeToastsMuted", () => {
  it("is muted until the deadline passes", () => {
    expect(isChangeToastsMuted(NOW + 1, NOW)).toBe(true);
    expect(isChangeToastsMuted(NOW, NOW)).toBe(false);
    expect(isChangeToastsMuted(NOW - 1, NOW)).toBe(false);
  });

  it("accepts every mute duration on offer", () => {
    for (const duration of MUTE_DURATIONS) {
      expect(isChangeToastsMuted(NOW + duration.ms, NOW)).toBe(true);
    }
  });

  it("keeps the longest mute when the clock steps back a little", () => {
    expect(isChangeToastsMuted(NOW + LONGEST_MUTE_MS, NOW - 5_000)).toBe(true);
  });

  it("ignores a deadline well beyond the longest mute", () => {
    expect(isChangeToastsMuted(NOW + LONGEST_MUTE_MS * 2, NOW)).toBe(false);
    expect(isChangeToastsMuted(Number.POSITIVE_INFINITY, NOW)).toBe(false);
  });

  it("ignores values that are not a timestamp", () => {
    expect(isChangeToastsMuted(null, NOW)).toBe(false);
    expect(isChangeToastsMuted(undefined, NOW)).toBe(false);
    expect(isChangeToastsMuted(Number.NaN, NOW)).toBe(false);
    expect(isChangeToastsMuted(String(NOW + 1000), NOW)).toBe(false);
  });
});

describe("changeToastStatus", () => {
  it("is on when enabled and not muted", () => {
    expect(
      changeToastStatus(
        { showChangeToasts: true, changeToastsMutedUntil: null },
        NOW,
      ),
    ).toBe("on");
  });

  it("is muted while the deadline is ahead, then on again", () => {
    const settings = {
      showChangeToasts: true,
      changeToastsMutedUntil: NOW + 60_000,
    };
    expect(changeToastStatus(settings, NOW)).toBe("muted");
    expect(changeToastStatus(settings, NOW + 60_000)).toBe("on");
  });

  it("is off when disabled, even with a mute pending", () => {
    expect(
      changeToastStatus(
        { showChangeToasts: false, changeToastsMutedUntil: NOW + 60_000 },
        NOW,
      ),
    ).toBe("off");
  });
});

describe("the port change toast", () => {
  it("has one id, distinct from sonner's own and from other toasts", () => {
    expect(isPortChangeToastId("port-changes")).toBe(true);
    expect(isPortChangeToastId(7)).toBe(false);
    expect(isPortChangeToastId("copied")).toBe(false);
  });

  it("describes a single change", () => {
    const shown = addPortChanges({ total: 0, latest: [] }, ["Port 3000 freed"]);
    expect(portChangeToastContent(shown)).toEqual({
      title: "Port change detected",
      description: "Port 3000 freed",
    });
  });

  it("adds later changes to what is already shown", () => {
    let shown = addPortChanges({ total: 0, latest: [] }, ["a", "b"]);
    shown = addPortChanges(shown, ["c"]);
    expect(portChangeToastContent(shown)).toEqual({
      title: "3 port changes detected",
      description: "a\nb\nc",
    });
  });

  it("keeps the latest lines and counts the earlier ones", () => {
    let shown = addPortChanges({ total: 0, latest: [] }, ["1", "2", "3", "4"]);
    shown = addPortChanges(shown, ["5", "6", "7"]);
    expect(shown.latest).toEqual(["3", "4", "5", "6", "7"]);
    expect(portChangeToastContent(shown)).toEqual({
      title: "7 port changes detected",
      description: "+2 earlier\n3\n4\n5\n6\n7",
    });
  });
});
