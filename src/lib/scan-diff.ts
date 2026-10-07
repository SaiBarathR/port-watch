import type { PortHistoryEvent } from "@/lib/port-history";
import type { PortProcess, RowChangeKind } from "@/lib/types";
import { portSignature } from "@/lib/types";

/** Everything that follows from one scan differing from the one before. */
export interface ScanDiff {
  /** Rows to highlight, by row id. Only rows the table shows. */
  rowChanges: Map<string, RowChangeKind>;
  /**
   * One line per port taken or freed, for the toast. Only rows shown, and
   * not the UDP sockets of a process that was there before and still is.
   */
  messages: string[];
  /** Every port taken or freed, shown or not, for the history. */
  historyEvents: PortHistoryEvent[];
  /** Watched ports that became busy, free, or changed hands. */
  watchedEvents: PortHistoryEvent[];
}

export interface ScanDiffOptions {
  /** Whether the table shows this process. Hidden ones are not announced. */
  isShown: (process: PortProcess) => boolean;
  watchedPorts: number[];
  /** ISO time to stamp the events with. */
  timestamp: string;
}

interface Holding {
  process: PortProcess;
  port: number;
  protocol: string;
}

// One entry per port a process holds, whatever the address: a server on
// 127.0.0.1 and [::1] holds the port once. Keyed by row id and not by PID,
// which listeners with no visible owner all share.
function holdings(processes: PortProcess[]): Map<string, Holding> {
  const held = new Map<string, Holding>();
  for (const process of processes) {
    for (const binding of process.ports) {
      const key = `${process.id}|${binding.protocol}|${binding.port}`;
      if (!held.has(key)) {
        held.set(key, {
          process,
          port: binding.port,
          protocol: binding.protocol,
        });
      }
    }
  }
  return held;
}

/**
 * What changed between two scans. One pass feeds the row highlights, the
 * toast, the history and the watched-port alerts, so they cannot disagree
 * about what happened.
 */
export function diffScans(
  prev: PortProcess[],
  next: PortProcess[],
  { isShown, watchedPorts, timestamp }: ScanDiffOptions,
): ScanDiff {
  const before = holdings(prev);
  const after = holdings(next);
  const event = (
    kind: PortHistoryEvent["kind"],
    { process, port, protocol }: Holding,
  ): PortHistoryEvent => ({
    timestamp,
    kind,
    port,
    protocol,
    pid: process.pid,
    processName: process.name,
  });

  const taken = [...after].filter(([key]) => !before.has(key));
  const freed = [...before].filter(([key]) => !after.has(key));

  const historyEvents = [
    ...taken.map(([, holding]) => event("occupied", holding)),
    ...freed.map(([, holding]) => event("freed", holding)),
  ];

  const prevById = new Map(prev.map((process) => [process.id, process]));
  const nextIds = new Set(next.map((process) => process.id));
  // A toast is for a server coming, going or taking another port. A browser
  // or a call opens and closes UDP sockets all day without doing any of
  // those; that is recorded in the history and marked on the row.
  const announced = ({ process, protocol }: Holding) =>
    isShown(process) &&
    !(
      protocol.toUpperCase() === "UDP" &&
      prevById.has(process.id) &&
      nextIds.has(process.id)
    );

  // A Set, because a port held over both TCP and UDP reads the same.
  const messages = new Set<string>();
  for (const [, holding] of taken) {
    if (announced(holding)) {
      const { process, port } = holding;
      messages.add(
        `Port ${port} is now in use by ${process.name} (PID ${process.pid})`,
      );
    }
  }
  for (const [, holding] of freed) {
    if (announced(holding)) {
      const { process, port } = holding;
      messages.add(`Port ${port} freed (${process.name}, PID ${process.pid})`);
    }
  }

  const rowChanges = new Map<string, RowChangeKind>();
  for (const process of next) {
    if (!isShown(process)) {
      continue;
    }
    const old = prevById.get(process.id);
    if (!old) {
      rowChanges.set(process.id, "new");
    } else if (portSignature(old) !== portSignature(process)) {
      rowChanges.set(process.id, "changed");
    }
  }

  return {
    rowChanges,
    messages: [...messages],
    historyEvents,
    watchedEvents: watchedPortEvents(before, after, watchedPorts, event),
  };
}

// A watched port is about the port, not about one process on it: several
// processes can share a socket, and a worker restarting is not news. It is
// taken when nobody held it, freed when nobody holds it any more, and has
// changed hands when none of its holders is one it had before.
function watchedPortEvents(
  before: Map<string, Holding>,
  after: Map<string, Holding>,
  watchedPorts: number[],
  event: (kind: PortHistoryEvent["kind"], holding: Holding) => PortHistoryEvent,
): PortHistoryEvent[] {
  const events: PortHistoryEvent[] = [];
  const holdersOf = (held: Map<string, Holding>, port: number) =>
    [...held.values()].filter((holding) => holding.port === port);

  for (const port of new Set(watchedPorts)) {
    const was = holdersOf(before, port);
    const now = holdersOf(after, port);
    if (now.length === 0) {
      if (was.length > 0) {
        events.push(event("freed", was[0]));
      }
      continue;
    }

    const heldBefore = new Set(was.map((holding) => holding.process.id));
    if (!now.some((holding) => heldBefore.has(holding.process.id))) {
      events.push(event("occupied", now[0]));
    }
  }

  return events;
}

/**
 * `next`, with every process that has not changed replaced by the object
 * `prev` holds for it, and `prev` itself when nothing changed at all. A row
 * whose process is the same object does not render again.
 */
export function shareUnchanged(
  prev: PortProcess[],
  next: PortProcess[],
): PortProcess[] {
  const prevById = new Map(prev.map((process) => [process.id, process]));
  let same = prev.length === next.length;

  const shared = next.map((process, index) => {
    const old = prevById.get(process.id);
    if (old && JSON.stringify(old) === JSON.stringify(process)) {
      same &&= prev[index] === old;
      return old;
    }
    same = false;
    return process;
  });

  return same ? prev : shared;
}
