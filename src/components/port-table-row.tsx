import { memo } from "react";
import { flexRender, type Cell, type Row } from "@tanstack/react-table";
import type { RowChangeKind } from "@/lib/types";
import type { PortProcess } from "@/lib/types";
import { cn } from "@/lib/utils";

// The tints are mixed into the background rather than laid over it: the
// first and last cells stay put while the others scroll beneath them, so
// they need the row's colour as an opaque one.
const ROW_TINT = {
  new: "[--row:color-mix(in_oklab,var(--color-emerald-500)_10%,var(--background))]",
  changed:
    "[--row:color-mix(in_oklab,var(--color-amber-500)_10%,var(--background))]",
} as const;

const ROW_BACKGROUND =
  "bg-(--row) [--row:var(--background)] hover:[--row:color-mix(in_oklch,var(--muted)_50%,var(--background))] data-[state=selected]:[--row:var(--accent)]";

// The select and actions columns stay in view while the rest scrolls sideways.
export function stickyCellClass(
  position: "first" | "last" | "corner-left" | "corner-right",
) {
  switch (position) {
    case "first":
      return "sticky left-0 z-10 bg-(--row)";
    case "last":
      return "sticky right-0 z-10 bg-(--row)";
    case "corner-left":
      return "sticky top-0 left-0 z-30 bg-background";
    case "corner-right":
      return "sticky top-0 right-0 z-30 bg-background";
  }
}

interface PortTableDataRowProps {
  row: Row<PortProcess>;
  // Not read here: the cells take settings from the table's `meta`, and
  // TanStack keeps `row` when only that changes. Without this prop an open
  // row menu kept showing the old settings until the next scan.
  meta: object;
  change: RowChangeKind | undefined;
  columnCount: number;
  // isSelected/canSelect are passed as primitives so this memoized row re-renders
  // when its selection state changes. TanStack reuses the same Row reference across
  // selection-only updates, so without these props the checkbox tick never updates.
  isSelected: boolean;
  canSelect: boolean;
}

export const PortTableDataRow = memo(function PortTableDataRow({
  row,
  change,
  columnCount,
  isSelected,
  canSelect,
}: PortTableDataRowProps) {
  return (
    <tr
      aria-selected={isSelected}
      data-state={isSelected ? "selected" : undefined}
      data-can-select={canSelect}
      className={cn(
        "group border-b transition-colors",
        ROW_BACKGROUND,
        change && ROW_TINT[change],
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
