import { memo, useState } from "react";
import { flexRender, type Cell, type Row } from "@tanstack/react-table";
import { ContextMenu, ContextMenuTrigger } from "@/components/ui/context-menu";
import { RowContextMenuContent } from "@/components/port-table-actions-cell";
import { useRefreshPause } from "@/lib/refresh-pause";
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
  /** The row Tab lands on. The arrow keys move it. */
  isTabStop: boolean;
  portIsShared: boolean;
}

export const PortTableDataRow = memo(function PortTableDataRow({
  row,
  change,
  columnCount,
  isSelected,
  canSelect,
  isTabStop,
  portIsShared,
}: PortTableDataRowProps) {
  const [menuOpen, setMenuOpen] = useState(false);
  // Rows must not move under an open menu.
  useRefreshPause("row-context-menu", menuOpen);

  return (
    <ContextMenu onOpenChange={setMenuOpen}>
      <ContextMenuTrigger asChild>
        <tr
          data-row-id={row.id}
          tabIndex={isTabStop ? 0 : -1}
          aria-selected={isSelected}
          data-state={isSelected ? "selected" : undefined}
          data-can-select={canSelect}
          className={cn(
            // No transition: the first and last cells are painted on their
            // own and would change a beat ahead of the rest of the row.
            "group border-b outline-none",
            ROW_BACKGROUND,
            change && ROW_TINT[change],
            // Drawn with the row's colour and a bar on its first cell: an
            // outline on a table row is not painted by every engine.
            "focus-visible:[--row:color-mix(in_oklab,var(--ring)_22%,var(--background))]",
          )}
        >
          {row
            .getVisibleCells()
            .map((cell: Cell<PortProcess, unknown>, index) => {
              const isFirst = index === 0;
              const isLast = index === columnCount - 1;
              const stickyClass = isFirst
                ? cn(
                    stickyCellClass("first"),
                    "group-focus-visible:shadow-[inset_3px_0_0_var(--ring)]",
                  )
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
      </ContextMenuTrigger>
      {menuOpen && (
        <RowContextMenuContent
          process={row.original}
          portIsShared={portIsShared}
        />
      )}
    </ContextMenu>
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
