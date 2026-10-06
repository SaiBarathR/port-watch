import type { AppSettings, PortProcess } from "@/lib/types";
import { groupDirectory, isPinned } from "@/lib/types";

type TableOrder = Pick<AppSettings, "pinnedPaths" | "groupByDirectory">;

/**
 * Table order: pinned projects first, then by directory when grouping, then
 * by first port.
 */
export function sortForTable(
  processes: PortProcess[],
  order: TableOrder,
): PortProcess[] {
  return [...processes].sort((a, b) => {
    const aPinned = isPinned(a, order.pinnedPaths);
    const bPinned = isPinned(b, order.pinnedPaths);
    if (aPinned !== bPinned) {
      return aPinned ? -1 : 1;
    }

    if (order.groupByDirectory) {
      const groupCompare = groupDirectory(a).localeCompare(groupDirectory(b));
      if (groupCompare !== 0) {
        return groupCompare;
      }
    }

    return (a.ports[0]?.port ?? 0) - (b.ports[0]?.port ?? 0);
  });
}

export type TableRowItem<Row> =
  { kind: "group"; id: string; label: string } | { kind: "data"; row: Row };

/**
 * Puts the section headers between rows that are already in table order.
 * Pinned rows get a "Pinned" header, and the rows after them an
 * "Other listeners" header, so they do not read as pinned too.
 */
export function withGroupHeaders<Row>(
  rows: Row[],
  processOf: (row: Row) => PortProcess,
  order: TableOrder,
): TableRowItem<Row>[] {
  const items: TableRowItem<Row>[] = [];
  let section: "pinned" | "others" | null = null;
  let lastGroup: string | null = null;

  for (const row of rows) {
    const process = processOf(row);
    const pinned = isPinned(process, order.pinnedPaths);

    if (pinned && section === null) {
      section = "pinned";
      items.push({ kind: "group", id: "group-pinned", label: "Pinned" });
    } else if (!pinned && section === "pinned") {
      section = "others";
      lastGroup = null;
      items.push({
        kind: "group",
        id: "group-others",
        label: "Other listeners",
      });
    }

    if (order.groupByDirectory) {
      const group = groupDirectory(process);
      if (group !== lastGroup) {
        lastGroup = group;
        items.push({
          kind: "group",
          id: `group-${section ?? "all"}-${group}`,
          label: group,
        });
      }
    }

    items.push({ kind: "data", row });
  }

  return items;
}
