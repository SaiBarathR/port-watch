export type PortHistoryEventKind = "occupied" | "freed";

export interface PortHistoryEvent {
  timestamp: string;
  kind: PortHistoryEventKind;
  port: number;
  protocol: string;
  pid: number;
  processName: string;
}

export interface PortSummary {
  port: number;
  protocol: string;
  firstSeen: string;
  lastSeen: string;
  eventCount: number;
  lastKind: PortHistoryEventKind;
  lastProcessName: string;
}

export type HistoryDayGroup = "today" | "yesterday" | "earlier";

export interface GroupedPortTimeline {
  today: PortHistoryEvent[];
  yesterday: PortHistoryEvent[];
  earlier: PortHistoryEvent[];
}

const HISTORY_KEY = "port-watch-history";
const MAX_EVENTS = 500;
const FLUSH_DELAY_MS = 500;

let cachedEvents: PortHistoryEvent[] | null = null;
const pendingEvents: PortHistoryEvent[] = [];
let flushTimer: number | null = null;

function loadRaw(): PortHistoryEvent[] {
  if (cachedEvents !== null) {
    return cachedEvents;
  }

  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    if (!raw) {
      cachedEvents = [];
      return cachedEvents;
    }
    const lines = raw.trim().split("\n").filter(Boolean);
    cachedEvents = lines.map((line) => JSON.parse(line) as PortHistoryEvent);
    return cachedEvents;
  } catch {
    cachedEvents = [];
    return cachedEvents;
  }
}

function saveRaw(events: PortHistoryEvent[]) {
  const trimmed = events.slice(-MAX_EVENTS);
  cachedEvents = trimmed;
  try {
    localStorage.setItem(
      HISTORY_KEY,
      trimmed.map((event) => JSON.stringify(event)).join("\n"),
    );
  } catch {
    // quota/storage failure — keep the in-memory cache for this session
  }
}

function flushPendingEvents() {
  flushTimer = null;
  if (pendingEvents.length === 0) {
    return;
  }

  const batch = pendingEvents.splice(0, pendingEvents.length);
  saveRaw([...loadRaw(), ...batch]);
}

function scheduleHistoryFlush() {
  if (flushTimer !== null) {
    return;
  }

  if (typeof requestIdleCallback === "function") {
    flushTimer = window.setTimeout(() => {
      requestIdleCallback(() => flushPendingEvents());
    }, FLUSH_DELAY_MS);
    return;
  }

  flushTimer = window.setTimeout(flushPendingEvents, FLUSH_DELAY_MS);
}

// Buffered events would otherwise be lost if the app quits within the
// flush window.
if (typeof window !== "undefined") {
  window.addEventListener("pagehide", () => {
    if (flushTimer !== null) {
      window.clearTimeout(flushTimer);
    }
    flushPendingEvents();
  });
}

export function getPortHistory(): PortHistoryEvent[] {
  return loadRaw().slice().reverse();
}

export function getPortTimeline(port: number): PortHistoryEvent[] {
  return loadRaw()
    .filter((event) => event.port === port)
    .reverse();
}

export function getPortSummaries(events: PortHistoryEvent[] = loadRaw()): PortSummary[] {
  // Key by protocol + port so TCP and UDP activity on the same port number
  // don't merge into one summary.
  const byPort = new Map<string, PortHistoryEvent[]>();

  for (const event of events) {
    const key = `${event.protocol}|${event.port}`;
    const list = byPort.get(key);
    if (list) {
      list.push(event);
    } else {
      byPort.set(key, [event]);
    }
  }

  const summaries: PortSummary[] = [];

  for (const portEvents of byPort.values()) {
    const sorted = [...portEvents].sort(
      (a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime(),
    );
    const first = sorted[0]!;
    const last = sorted[sorted.length - 1]!;

    summaries.push({
      port: last.port,
      protocol: last.protocol,
      firstSeen: first.timestamp,
      lastSeen: last.timestamp,
      eventCount: sorted.length,
      lastKind: last.kind,
      lastProcessName: last.processName,
    });
  }

  return summaries.sort(
    (a, b) => new Date(b.lastSeen).getTime() - new Date(a.lastSeen).getTime(),
  );
}

export function getPortSummary(port: number): PortSummary | null {
  return getPortSummaries().find((summary) => summary.port === port) ?? null;
}

function startOfLocalDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

export function historyDayGroup(
  timestamp: string,
  now = new Date(),
): HistoryDayGroup {
  const eventDate = new Date(timestamp);
  const todayStart = startOfLocalDay(now);
  const yesterdayStart = new Date(todayStart);
  yesterdayStart.setDate(yesterdayStart.getDate() - 1);

  if (eventDate >= todayStart) {
    return "today";
  }
  if (eventDate >= yesterdayStart) {
    return "yesterday";
  }
  return "earlier";
}

export function groupTimelineByDay(
  events: PortHistoryEvent[],
  now = new Date(),
): GroupedPortTimeline {
  const grouped: GroupedPortTimeline = {
    today: [],
    yesterday: [],
    earlier: [],
  };

  for (const event of events) {
    grouped[historyDayGroup(event.timestamp, now)].push(event);
  }

  return grouped;
}

export function appendPortHistoryEvents(events: PortHistoryEvent[]) {
  if (events.length === 0) {
    return;
  }

  pendingEvents.push(...events);
  scheduleHistoryFlush();
}

export function clearPortHistory() {
  pendingEvents.length = 0;
  if (flushTimer !== null) {
    window.clearTimeout(flushTimer);
    flushTimer = null;
  }
  cachedEvents = [];
  localStorage.removeItem(HISTORY_KEY);
}

export function formatHistoryTime(timestamp: string): string {
  return new Date(timestamp).toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });
}

export function formatHistoryDayLabel(
  timestamp: string,
  now = new Date(),
): string {
  const group = historyDayGroup(timestamp, now);
  if (group === "today") {
    return "Today";
  }
  if (group === "yesterday") {
    return "Yesterday";
  }

  const date = new Date(timestamp);
  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: date.getFullYear() !== now.getFullYear() ? "numeric" : undefined,
  });
}

export function formatHistorySeen(timestamp: string, now = new Date()): string {
  const group = historyDayGroup(timestamp, now);
  const time = formatHistoryTime(timestamp);
  if (group === "today") {
    return `today ${time}`;
  }
  if (group === "yesterday") {
    return `yesterday ${time}`;
  }
  return `${formatHistoryDayLabel(timestamp, now)} ${time}`;
}

export function formatHistoryEvent(event: PortHistoryEvent): string {
  const time = new Date(event.timestamp).toLocaleString();
  const action = event.kind === "occupied" ? "occupied by" : "freed from";
  return `${time} — Port ${event.port}/${event.protocol.toLowerCase()} ${action} ${event.processName} (PID ${event.pid})`;
}

export function formatHistoryEventShort(event: PortHistoryEvent): string {
  const action = event.kind === "occupied" ? "occupied" : "freed";
  return `${formatHistoryTime(event.timestamp)} — ${action} by ${event.processName} (PID ${event.pid})`;
}
