import { memo } from "react";
import { flexRender, type Cell, type Row } from "@tanstack/react-table";
import type { RowChangeKind } from "@/lib/types";
import type { PortProcess } from "@/lib/types";
import { cn } from "@/lib/utils";

function changeRowClass(change: RowChangeKind | undefined): string {
  switch (change) {
    case "new":
      return "bg-emerald-500/10 hover:bg-emerald-500/15";
    case "changed":
      return "bg-amber-500/10 hover:bg-amber-500/15";
    default:
      return "";
  }
}

function stickyCellClass(position: "first" | "last") {
  const base =
    "bg-background group-hover:bg-[color-mix(in_oklch,var(--muted)_50%,var(--background))]";
  switch (position) {
    case "first":
      return cn(base, "sticky left-0 z-10");
    case "last":
      return cn(base, "sticky right-0 z-10");
  }
}

interface PortTableDataRowProps {
  row: Row<PortProcess>;
  change: RowChangeKind | undefined;
  columnCount: number;
}

export const PortTableDataRow = memo(function PortTableDataRow({
  row,
  change,
  columnCount,
}: PortTableDataRowProps) {
  return (
    <tr
      className={cn(
        "group border-b transition-colors hover:bg-muted/50",
        changeRowClass(change),
      )}
    >
      {row.getVisibleCells().map((cell: Cell<PortProcess, unknown>, index) => {
        const isFirst = index === 0;
        const isLast = index === columnCount - 1;
        const stickyClass = isFirst
          ? stickyCellClass("first")
          : isLast
            ? stickyCellClass("last")
            : undefined;

        return (
          <td
            key={cell.id}
            className={cn(
              "overflow-hidden p-2 align-middle whitespace-nowrap",
              stickyClass,
            )}
          >
            {flexRender(cell.column.columnDef.cell, cell.getContext())}
          </td>
        );
      })}
    </tr>
  );
});

interface PortTableGroupRowProps {
  id: string;
  label: string;
  columnCount: number;
}

export const PortTableGroupRow = memo(function PortTableGroupRow({
  id,
  label,
  columnCount,
}: PortTableGroupRowProps) {
  return (
    <tr key={id} className="border-b bg-muted/30">
      <td
        colSpan={columnCount}
        className="px-2 py-1.5 text-xs font-medium text-muted-foreground"
      >
        {label}
      </td>
    </tr>
  );
});
