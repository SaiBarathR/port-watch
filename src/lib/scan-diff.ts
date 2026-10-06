import type { PortProcess, RowChangeKind } from "@/lib/types";
import { portSignature } from "@/lib/types";

export interface ProcessChanges {
  /** Rows to highlight, by row id. */
  rowChanges: Map<string, RowChangeKind>;
  /** One line per port taken or freed, for the port change toast. */
  messages: string[];
}

/**
 * What changed between two scans, for the row highlights and the toast. Pass
 * only the processes the user has chosen to see: a change in a hidden one is
 * not something to announce.
 */
export function diffProcesses(
  prev: PortProcess[],
  next: PortProcess[],
): ProcessChanges {
  const prevById = new Map(prev.map((process) => [process.id, process]));
  const rowChanges = new Map<string, RowChangeKind>();
  const messages: string[] = [];

  for (const process of next) {
    const old = prevById.get(process.id);
    if (!old) {
      rowChanges.set(process.id, "new");
      for (const binding of process.ports) {
        messages.push(
          `Port ${binding.port} is now in use by ${process.name} (PID ${process.pid})`,
        );
      }
    } else if (portSignature(old) !== portSignature(process)) {
      rowChanges.set(process.id, "changed");
    }
    prevById.delete(process.id);
  }

  for (const gone of prevById.values()) {
    for (const binding of gone.ports) {
      messages.push(
        `Port ${binding.port} freed (${gone.name}, PID ${gone.pid})`,
      );
    }
  }

  return { rowChanges, messages };
}
