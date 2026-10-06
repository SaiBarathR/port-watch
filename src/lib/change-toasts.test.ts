import { describe, expect, it } from "vitest";
import {
  MUTE_DURATIONS,
  changeToastStatus,
  isChangeToastsMuted,
  isPortChangeToastId,
  nextPortChangeToastId,
} from "./change-toasts";

const NOW = Date.UTC(2026, 9, 6, 12, 0, 0);
const LONGEST_MUTE_MS = Math.max(...MUTE_DURATIONS.map((duration) => duration.ms));

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

  it("ignores a deadline further out than the longest mute", () => {
    expect(isChangeToastsMuted(NOW + LONGEST_MUTE_MS + 1, NOW)).toBe(false);
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

describe("port change toast ids", () => {
  it("are unique and recognisable", () => {
    const first = nextPortChangeToastId();
    const second = nextPortChangeToastId();
    expect(first).not.toBe(second);
    expect(isPortChangeToastId(first)).toBe(true);
    expect(isPortChangeToastId(second)).toBe(true);
  });

  it("does not match sonner's own numeric ids or other toasts", () => {
    expect(isPortChangeToastId(7)).toBe(false);
    expect(isPortChangeToastId("copied")).toBe(false);
  });
});
