import { useEffect, useMemo, useRef, useState } from "react";
import {
  flexRender,
  getCoreRowModel,
  useReactTable,
  type ColumnSizingState,
  type RowSelectionState,
} from "@tanstack/react-table";
import { Button } from "@/components/ui/button";
import { TooltipProvider } from "@/components/ui/tooltip";
import { OpenMenuProvider } from "@/components/port-table-actions-cell";
import { columns, type PortTableMeta } from "@/components/port-table-columns";
import {
  PortTableDataRow,
  PortTableGroupRow,
  stickyCellClass,
} from "@/components/port-table-row";
import { useProcessActions } from "@/components/process-actions";
import { useRowActionRunner } from "@/hooks/use-row-actions";
import {
  DEFAULT_COLUMN_SIZING,
  loadColumnSizing,
  saveColumnSizing,
} from "@/lib/column-sizing";
import { isMacOS } from "@/lib/platform";
import { focusRow } from "@/lib/row-focus";
import { useRefreshPause } from "@/lib/refresh-pause";
import { sortForTable, withGroupHeaders } from "@/lib/table-rows";
import type { AppSettings, PortProcess, RowChangeKind } from "@/lib/types";
import { cn } from "@/lib/utils";

interface PortTableProps {
  /** Every listener, to tell "nothing listening" from "nothing matches". */
  processes: PortProcess[];
  /** The ones the search and the filters let through, which are the rows. */
  shownProcesses: PortProcess[];
  /** True until the first scan has come back. */
  loading: boolean;
  settings: AppSettings;
  rowChanges: Map<string, RowChangeKind>;
}

export function PortTable({
  processes,
  shownProcesses,
  loading,
  settings,
  rowChanges,
}: PortTableProps) {
  const { canStop, stop } = useProcessActions();
  const [rowSelection, setRowSelection] = useState<RowSelectionState>({});
  const [columnSizing, setColumnSizing] =
    useState<ColumnSizingState>(loadColumnSizing);
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);

  // Rows must not move under an open menu.
  useRefreshPause("row-menu", openMenuId !== null);

  // A process that is gone, or can no longer be stopped, leaves the selection.
  useEffect(() => {
    setRowSelection((current) => {
      const selectableIds = new Set(
        processes.filter(canStop).map((process) => process.id),
      );
      const next: RowSelectionState = {};
      let changed = false;
      for (const [id, selected] of Object.entries(current)) {
        if (selectableIds.has(id)) {
          next[id] = selected;
        } else {
          changed = true;
        }
      }
      return changed ? next : current;
    });
  }, [processes, canStop]);

  const actionSettings = useMemo(
    () => ({
      pinnedPaths: settings.pinnedPaths,
      preferredEditor: settings.preferredEditor,
      useHttpsForLocalhost: settings.useHttpsForLocalhost,
    }),
    [
      settings.pinnedPaths,
      settings.preferredEditor,
      settings.useHttpsForLocalhost,
    ],
  );

  const tableData = useMemo(
    () =>
      sortForTable(shownProcesses, {
        pinnedPaths: settings.pinnedPaths,
        groupByDirectory: settings.groupByDirectory,
      }),
    [shownProcesses, settings.groupByDirectory, settings.pinnedPaths],
  );

  // A menu goes with its row. Nothing tells this table that one closed
  // because its row left, and the scans it was holding still would never
  // start again.
  if (openMenuId !== null && !tableData.some((row) => row.id === openMenuId)) {
    setOpenMenuId(null);
  }

  // A socket can be shared (a master and its workers). For such a row,
  // stopping it and freeing its port are different things.
  const isPortShared = useMemo(() => {
    const holders = new Map<number, number>();
    for (const process of processes) {
      for (const port of new Set(process.ports.map((held) => held.port))) {
        holders.set(port, (holders.get(port) ?? 0) + 1);
      }
    }
    return (process: PortProcess) =>
      (holders.get(process.ports[0]?.port ?? -1) ?? 0) > 1;
  }, [processes]);

  const portsColumnWidth = columnSizing.ports ?? DEFAULT_COLUMN_SIZING.ports;
  const meta = useMemo<PortTableMeta>(
    () => ({
      portsColumnWidth,
      rowChanges,
      canStop,
      isPortShared,
      actionSettings,
    }),
    [actionSettings, canStop, isPortShared, portsColumnWidth, rowChanges],
  );

  // With system services hidden every row is the current user's own
  // process: two columns that would say the same thing on every line.
  const columnVisibility = useMemo(
    () => ({
      user: !settings.hideSystemServices,
      type: !settings.hideSystemServices,
    }),
    [settings.hideSystemServices],
  );

  const table = useReactTable({
    data: tableData,
    columns,
    meta,
    state: { columnSizing, columnVisibility, rowSelection },
    onColumnSizingChange: setColumnSizing,
    onRowSelectionChange: setRowSelection,
    enableRowSelection: (row) => canStop(row.original),
    getRowId: (row) => row.id,
    columnResizeMode: "onChange",
    enableColumnResizing: true,
    getCoreRowModel: getCoreRowModel(),
  });

  // Rows must not move while a column is being dragged either, and the
  // widths are saved when the drag ends, not on every pixel of it.
  const resizing = table.getState().columnSizingInfo.isResizingColumn !== false;
  useRefreshPause("column-resize", resizing);
  // TanStack ends a touch drag on touchend only. A drag the system takes
  // over (touchcancel) would otherwise stay "in progress", with scans paused.
  useEffect(() => {
    if (!resizing) {
      return;
    }
    const cancel = () => table.resetHeaderSizeInfo(true);
    document.addEventListener("touchcancel", cancel);
    return () => document.removeEventListener("touchcancel", cancel);
  }, [resizing, table]);
  const savedSizingRef = useRef(columnSizing);
  useEffect(() => {
    if (!resizing && savedSizingRef.current !== columnSizing) {
      savedSizingRef.current = columnSizing;
      saveColumnSizing(columnSizing);
    }
  }, [resizing, columnSizing]);

  // Derived on every render: `table` keeps its identity across state changes,
  // so it cannot serve as a memo dependency.
  const selectedProcesses = table
    .getSelectedRowModel()
    .rows.map((row) => row.original);

  const columnCount = table.getVisibleLeafColumns().length;

  // The table is one tab stop. Tab lands on a row; the arrow keys move from
  // row to row, and the row's own keys act on it. Before this, each row was
  // three tab stops: its checkbox, its folder link and its menu button.
  const rows = table.getRowModel().rows;
  const [activeRowId, setActiveRowId] = useState<string | null>(null);
  const tabStopId = rows.some((row) => row.id === activeRowId)
    ? activeRowId
    : (rows[0]?.id ?? null);
  const run = useRowActionRunner();

  const onRowKeyDown = (event: React.KeyboardEvent<HTMLElement>) => {
    const target = event.target as HTMLElement;
    // A control inside the row has the key, not the row.
    if (target.tagName !== "TR") {
      return;
    }
    const index = rows.findIndex((row) => row.id === target.dataset.rowId);
    if (index < 0) {
      return;
    }
    const row = rows[index];
    const process = row.original;
    const shared = isPortShared(process);
    const mod = isMacOS() ? event.metaKey : event.ctrlKey;
    const key = event.key.length === 1 ? event.key.toLowerCase() : event.key;
    const moveTo = (to: number) =>
      focusRow(rows[Math.max(0, Math.min(rows.length - 1, to))].id);

    if (key === "ArrowDown") {
      moveTo(index + 1);
    } else if (key === "ArrowUp") {
      moveTo(index - 1);
    } else if (key === "Home") {
      moveTo(0);
    } else if (key === "End") {
      moveTo(rows.length - 1);
    } else if (key === " ") {
      if (row.getCanSelect()) {
        row.toggleSelected();
      }
    } else if (key === "Enter" && !mod && !event.altKey) {
      run("open-browser", process, shared);
    } else if (mod && key === "o") {
      run("open-editor", process, shared);
    } else if (mod && event.shiftKey && key === "c") {
      run("copy-url", process, shared);
    } else if (mod && (key === "Backspace" || key === "Delete")) {
      run("stop", process, shared);
    } else if (
      (mod && key === "k") ||
      key === "ContextMenu" ||
      (event.shiftKey && key === "F10")
    ) {
      setOpenMenuId(row.id);
    } else {
      return;
    }
    event.preventDefault();
    // The toolbar's own shortcuts are for when no row has the keys.
    event.stopPropagation();
  };

  const tableRows = withGroupHeaders(
    table.getRowModel().rows,
    (row) => row.original,
    settings,
  );

  return (
    <OpenMenuProvider openMenuId={openMenuId} setOpenMenuId={setOpenMenuId}>
      <TooltipProvider>
        <div
          className={cn(
            "relative flex h-full min-h-0 flex-col",
            resizing && "cursor-col-resize select-none",
          )}
        >
          <div className="min-h-0 flex-1 overflow-auto rounded-md border">
            <table
              className="caption-bottom text-sm"
              style={{
                width: table.getTotalSize(),
                minWidth: "100%",
                tableLayout: "fixed",
              }}
            >
              <colgroup>
                {table.getVisibleLeafColumns().map((column) => (
                  <col key={column.id} style={{ width: column.getSize() }} />
                ))}
              </colgroup>
              <thead className="sticky top-0 z-20 bg-background [&_tr]:border-b">
                {table.getHeaderGroups().map((headerGroup) => (
                  <tr key={headerGroup.id} className="border-b">
                    {headerGroup.headers.map((header, index) => {
                      const isFirst = index === 0;
                      const isLast = index === columnCount - 1;
                      const stickyClass = isFirst
                        ? stickyCellClass("corner-left")
                        : isLast
                          ? stickyCellClass("corner-right")
                          : "sticky top-0 z-20 bg-background";

                      return (
                        <th
                          key={header.id}
                          className={cn(
                            "relative h-10 border-r px-2 text-left align-middle font-medium whitespace-nowrap text-foreground last:border-r-0",
                            stickyClass,
                          )}
                        >
                          {header.isPlaceholder
                            ? null
                            : flexRender(
                                header.column.columnDef.header,
                                header.getContext(),
                              )}
                          {header.column.getCanResize() && (
                            <div
                              onMouseDown={header.getResizeHandler()}
                              onTouchStart={header.getResizeHandler()}
                              onDoubleClick={() => header.column.resetSize()}
                              className="group/resize absolute top-0 -right-1 z-40 h-full w-2 cursor-col-resize touch-none select-none"
                            >
                              <div
                                className={cn(
                                  "absolute top-0 left-1/2 h-full w-px -translate-x-1/2 bg-border opacity-0 transition-opacity group-hover/resize:opacity-100",
                                  header.column.getIsResizing() &&
                                    "bg-primary opacity-100",
                                )}
                              />
                            </div>
                          )}
                        </th>
                      );
                    })}
                  </tr>
                ))}
              </thead>
              <tbody
                className="[&_tr:last-child]:border-0"
                onKeyDown={onRowKeyDown}
                onFocus={(event) => {
                  const row = (
                    event.target as HTMLElement
                  ).closest<HTMLElement>("tr[data-row-id]");
                  if (row?.dataset.rowId) {
                    setActiveRowId(row.dataset.rowId);
                  }
                }}
              >
                {tableRows.length ? (
                  tableRows.map((item) => {
                    if (item.kind === "group") {
                      return (
                        <PortTableGroupRow
                          key={item.id}
                          id={item.id}
                          label={item.label}
                          columnCount={columnCount}
                        />
                      );
                    }

                    return (
                      <PortTableDataRow
                        key={item.row.id}
                        row={item.row}
                        meta={meta}
                        isSelected={item.row.getIsSelected()}
                        canSelect={item.row.getCanSelect()}
                        isTabStop={item.row.id === tabStopId}
                        portIsShared={isPortShared(item.row.original)}
                        change={rowChanges.get(item.row.original.id)}
                        columnCount={columnCount}
                      />
                    );
                  })
                ) : (
                  <tr className="border-b">
                    <td
                      colSpan={columnCount}
                      className="h-24 p-2 text-center align-middle"
                    >
                      {loading ? (
                        <span className="text-muted-foreground">
                          Scanning ports…
                        </span>
                      ) : processes.length > 0 ? (
                        <span className="text-muted-foreground">
                          No listeners match your current search or filters.
                        </span>
                      ) : (
                        "No listening ports found."
                      )}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
            {/* Room to scroll the last rows clear of the bar below. */}
            {selectedProcesses.length > 0 && <div className="h-14" />}
          </div>

          {/* Over the table, not above it: ticking the first checkbox used
              to push every row down under the pointer. */}
          {selectedProcesses.length > 0 && (
            <div
              role="toolbar"
              aria-label="Selected processes"
              className="absolute bottom-4 left-1/2 z-30 flex -translate-x-1/2 items-center gap-1 rounded-full border bg-popover py-1 pr-1 pl-4 text-sm text-popover-foreground shadow-lg"
            >
              <span className="mr-2 whitespace-nowrap tabular-nums">
                {selectedProcesses.length} selected
              </span>
              <Button
                size="sm"
                variant="destructive"
                className="h-7 rounded-full"
                onClick={() =>
                  stop(
                    selectedProcesses,
                    selectedProcesses.length === 1
                      ? undefined
                      : `Stop ${selectedProcesses.length} selected processes?`,
                  )
                }
              >
                Stop
              </Button>
              <Button
                size="sm"
                variant="ghost"
                className="h-7 rounded-full"
                onClick={() => setRowSelection({})}
              >
                Clear
              </Button>
            </div>
          )}
        </div>
      </TooltipProvider>
    </OpenMenuProvider>
  );
}
