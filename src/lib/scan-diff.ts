import type { PortProcess, RowChangeKind } from "@/lib/types";
import { portSignature } from "@/lib/types";

export interface ProcessChanges {
  /** Rows to highlight, by PID. */
  rowChanges: Map<number, RowChangeKind>;
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
  const prevByPid = new Map(prev.map((process) => [process.pid, process]));
  const rowChanges = new Map<number, RowChangeKind>();
  const messages: string[] = [];

  for (const process of next) {
    const old = prevByPid.get(process.pid);
    if (!old) {
      rowChanges.set(process.pid, "new");
      for (const binding of process.ports) {
        messages.push(
          `Port ${binding.port} is now in use by ${process.name} (PID ${process.pid})`,
        );
      }
    } else if (portSignature(old) !== portSignature(process)) {
      rowChanges.set(process.pid, "changed");
    }
    prevByPid.delete(process.pid);
  }

  for (const gone of prevByPid.values()) {
    for (const binding of gone.ports) {
      messages.push(
        `Port ${binding.port} freed (${gone.name}, PID ${gone.pid})`,
      );
    }
  }

  return { rowChanges, messages };
}
