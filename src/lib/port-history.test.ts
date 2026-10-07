// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PortHistoryEvent } from "@/lib/port-history";

const HISTORY_KEY = "port-watch-history";

function event(
  port: number,
  overrides: Partial<PortHistoryEvent> = {},
): PortHistoryEvent {
  return {
    timestamp: "2026-10-06T10:00:00.000Z",
    kind: "occupied",
    port,
    protocol: "TCP",
    pid: 4242,
    processName: "node",
    ...overrides,
  };
}

// The module keeps what it has read in memory. Each test loads it afresh, as
// a launch does.
async function history() {
  vi.resetModules();
  return import("@/lib/port-history");
}

const stored = () =>
  (localStorage.getItem(HISTORY_KEY) ?? "")
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line) as PortHistoryEvent);

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("port history", () => {
  it("is written a moment after a change, several changes at once", async () => {
    const { appendPortHistoryEvents } = await history();
    const write = vi.spyOn(Storage.prototype, "setItem");

    appendPortHistoryEvents([event(3000)]);
    appendPortHistoryEvents([event(4000)]);
    expect(stored()).toEqual([]);

    vi.runAllTimers();

    expect(stored().map((entry) => entry.port)).toEqual([3000, 4000]);
    expect(write).toHaveBeenCalledTimes(1);
  });

  it("is written at once when the window goes away", async () => {
    const { appendPortHistoryEvents } = await history();

    appendPortHistoryEvents([event(3000)]);
    window.dispatchEvent(new Event("pagehide"));

    expect(stored().map((entry) => entry.port)).toEqual([3000]);
  });

  it("keeps the newest 500 events", async () => {
    const { appendPortHistoryEvents, getPortSummaries } = await history();

    appendPortHistoryEvents(
      Array.from({ length: 510 }, (_, index) => event(1000 + index)),
    );
    vi.runAllTimers();

    expect(stored()).toHaveLength(500);
    expect(stored()[0].port).toBe(1010);
    expect(getPortSummaries()).toHaveLength(500);
  });

  it("adds to what an earlier session stored", async () => {
    localStorage.setItem(HISTORY_KEY, JSON.stringify(event(3000)));
    const { appendPortHistoryEvents, getPortTimeline } = await history();

    appendPortHistoryEvents([event(3000, { kind: "freed" })]);
    vi.runAllTimers();

    // Newest first.
    expect(getPortTimeline(3000).map((entry) => entry.kind)).toEqual([
      "freed",
      "occupied",
    ]);
    expect(stored()).toHaveLength(2);
  });

  it("loses only the lines it cannot read", async () => {
    localStorage.setItem(
      HISTORY_KEY,
      [
        JSON.stringify(event(3000)),
        '{"timestamp":"2026-10-06T10:0',
        "null",
        JSON.stringify({ ...event(4000), port: "4000" }),
        JSON.stringify(event(5000)),
      ].join("\n"),
    );
    const { getPortSummaries } = await history();

    expect(
      getPortSummaries()
        .map((summary) => summary.port)
        .sort(),
    ).toEqual([3000, 5000]);
  });

  it("lasts for the session when it cannot be stored", async () => {
    const { appendPortHistoryEvents, getPortTimeline } = await history();
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("full", "QuotaExceededError");
    });

    appendPortHistoryEvents([event(3000)]);
    expect(() => vi.runAllTimers()).not.toThrow();

    expect(getPortTimeline(3000)).toHaveLength(1);
  });

  it("starts empty when storage cannot be read", async () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("denied", "SecurityError");
    });
    const { getPortSummaries } = await history();

    expect(getPortSummaries()).toEqual([]);
  });

  it("forgets what was waiting to be written when it is cleared", async () => {
    localStorage.setItem(HISTORY_KEY, JSON.stringify(event(3000)));
    const { appendPortHistoryEvents, clearPortHistory, getPortSummaries } =
      await history();

    appendPortHistoryEvents([event(4000)]);
    clearPortHistory();
    vi.runAllTimers();

    expect(getPortSummaries()).toEqual([]);
    expect(localStorage.getItem(HISTORY_KEY)).toBeNull();
  });
});

describe("port summaries", () => {
  it("keep TCP and UDP on the same port number apart", async () => {
    const { getPortSummaries } = await history();

    const summaries = getPortSummaries([
      event(5353, { protocol: "UDP", processName: "mDNSResponder" }),
      event(5353, { protocol: "TCP" }),
    ]);

    expect(summaries.map((summary) => summary.protocol).sort()).toEqual([
      "TCP",
      "UDP",
    ]);
  });

  it("and timelines can be asked for one protocol of a port", async () => {
    localStorage.setItem(
      HISTORY_KEY,
      [
        event(5353, { protocol: "UDP", processName: "mDNSResponder" }),
        event(5353, { protocol: "TCP" }),
        event(5353, { protocol: "UDP", kind: "freed" }),
      ]
        .map((entry) => JSON.stringify(entry))
        .join("\n"),
    );
    const { getPortSummary, getPortTimeline } = await history();

    expect(getPortTimeline(5353)).toHaveLength(3);
    expect(getPortTimeline(5353, "UDP").map((entry) => entry.kind)).toEqual([
      "freed",
      "occupied",
    ]);
    expect(getPortTimeline(5353, "TCP")).toHaveLength(1);
    expect(getPortSummary(5353, "UDP")?.eventCount).toBe(2);
    expect(getPortSummary(5353, "TCP")?.eventCount).toBe(1);
    expect(getPortSummary(5353, "SCTP")).toBeNull();
  });

  it("say when a port was first and last seen, and by whom, newest port first", async () => {
    const { getPortSummaries } = await history();

    const summaries = getPortSummaries([
      event(3000, { timestamp: "2026-10-06T09:00:00.000Z" }),
      event(8080, { timestamp: "2026-10-06T09:30:00.000Z" }),
      event(3000, {
        timestamp: "2026-10-06T11:00:00.000Z",
        kind: "freed",
        processName: "vite",
      }),
    ]);

    expect(summaries).toEqual([
      {
        port: 3000,
        protocol: "TCP",
        firstSeen: "2026-10-06T09:00:00.000Z",
        lastSeen: "2026-10-06T11:00:00.000Z",
        eventCount: 2,
        lastKind: "freed",
        lastProcessName: "vite",
      },
      expect.objectContaining({ port: 8080, eventCount: 1 }),
    ]);
  });
});

describe("history days", () => {
  it("are told apart at local midnight", async () => {
    const { historyDayGroup, groupTimelineByDay } = await history();
    const now = new Date(2026, 9, 6, 15, 0);
    const at = (...time: [number, number, number, number, number]) =>
      new Date(...time).toISOString();

    expect(historyDayGroup(at(2026, 9, 6, 0, 0), now)).toBe("today");
    expect(historyDayGroup(at(2026, 9, 5, 23, 59), now)).toBe("yesterday");
    expect(historyDayGroup(at(2026, 9, 5, 0, 0), now)).toBe("yesterday");
    expect(historyDayGroup(at(2026, 9, 4, 23, 59), now)).toBe("earlier");

    const grouped = groupTimelineByDay(
      [
        event(1, { timestamp: at(2026, 9, 6, 9, 0) }),
        event(2, { timestamp: at(2026, 9, 5, 9, 0) }),
        event(3, { timestamp: at(2026, 8, 1, 9, 0) }),
      ],
      now,
    );
    expect(grouped.today.map((entry) => entry.port)).toEqual([1]);
    expect(grouped.yesterday.map((entry) => entry.port)).toEqual([2]);
    expect(grouped.earlier.map((entry) => entry.port)).toEqual([3]);
  });
});
