import { beforeEach, describe, expect, it, vi } from "vitest";

const sonner = vi.hoisted(() => ({
  info: vi.fn(),
  dismiss: vi.fn(),
  getToasts: vi.fn((): { id: number | string }[] => []),
}));

vi.mock("sonner", () => ({ toast: sonner }));

import {
  MUTE_DURATIONS,
  VISIBLE_TOASTS,
  changeToastStatus,
  isChangeToastsMuted,
  addPortChanges,
  dismissPortChangeToasts,
  isPortChangeToastId,
  portChangeToastContent,
  showPortChanges,
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
  it("has ids distinct from sonner's own and from other toasts", () => {
    expect(isPortChangeToastId("port-change-1")).toBe(true);
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

describe("showPortChanges", () => {
  beforeEach(() => {
    dismissPortChangeToasts();
    sonner.info.mockClear();
    sonner.dismiss.mockClear();
  });

  const lastCall = () => {
    const calls = sonner.info.mock.calls;
    const [title, options] = calls[calls.length - 1];
    return { title, ...options };
  };

  it("updates the toast on screen instead of adding another", () => {
    showPortChanges(["a"]);
    const first = lastCall();
    showPortChanges(["b"]);
    const second = lastCall();

    expect(first.title).toBe("Port change detected");
    expect(second.title).toBe("2 port changes detected");
    expect(second.id).toBe(first.id);
    expect(isPortChangeToastId(first.id)).toBe(true);
  });

  // sonner removes a toast by id about 200 ms after it starts to leave. An
  // update under the same id in that window would vanish with it.
  it.each(["onAutoClose", "onDismiss"] as const)(
    "starts a new toast once the previous one began to leave (%s)",
    (ending) => {
      showPortChanges(["a"]);
      const first = lastCall();
      first[ending]();

      showPortChanges(["b"]);
      const second = lastCall();

      expect(second.id).not.toBe(first.id);
      expect(second.title).toBe("Port change detected");
      expect(second.description).toBe("b");
    },
  );

  it("ignores a late callback from a toast that was already replaced", () => {
    showPortChanges(["a"]);
    const first = lastCall();
    first.onAutoClose();
    showPortChanges(["b"]);
    const second = lastCall();

    first.onDismiss();
    showPortChanges(["c"]);

    expect(lastCall().id).toBe(second.id);
    expect(lastCall().title).toBe("2 port changes detected");
  });

  // Updating a toast leaves it where it is in the stack, which shows only
  // the newest few.
  it("moves to a new toast, keeping the count, when newer toasts buried it", () => {
    showPortChanges(["a", "b"]);
    const first = lastCall();
    const newer = Array.from({ length: VISIBLE_TOASTS }, (_, n) => ({ id: n }));
    sonner.getToasts.mockReturnValueOnce([{ id: first.id }, ...newer]);

    showPortChanges(["c"]);

    expect(lastCall().id).not.toBe(first.id);
    expect(lastCall().title).toBe("3 port changes detected");
    expect(sonner.dismiss.mock.calls).toEqual([[first.id]]);

    // The buried toast reporting its dismissal does not reset the new one.
    first.onDismiss();
    showPortChanges(["d"]);
    expect(lastCall().title).toBe("4 port changes detected");
  });

  it("stays in place while it is still among the visible toasts", () => {
    showPortChanges(["a"]);
    const first = lastCall();
    const newer = Array.from({ length: VISIBLE_TOASTS - 1 }, (_, n) => ({
      id: n,
    }));
    sonner.getToasts.mockReturnValueOnce([{ id: first.id }, ...newer]);

    showPortChanges(["b"]);

    expect(lastCall().id).toBe(first.id);
    expect(sonner.dismiss).not.toHaveBeenCalled();
  });

  it("starts a new toast after the port change toasts are dismissed", () => {
    showPortChanges(["a"]);
    const first = lastCall();
    sonner.getToasts.mockReturnValueOnce([{ id: first.id }, { id: "copied" }]);

    dismissPortChangeToasts();
    showPortChanges(["b"]);

    expect(sonner.dismiss.mock.calls).toEqual([[first.id]]);
    expect(lastCall().id).not.toBe(first.id);
    expect(lastCall().title).toBe("Port change detected");
  });
});
