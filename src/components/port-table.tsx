import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
  type ColumnSizingState,
  type Header,
  type Row,
  type RowSelectionState,
  type SortingState,
} from "@tanstack/react-table";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { DeleteDialog } from "@/components/delete-dialog";
import {
  OpenMenuProvider,
  PortTableActionsCell,
  type PortTableActionsHandlers,
} from "@/components/port-table-actions-cell";
import { PortTableDataRow, PortTableGroupRow } from "@/components/port-table-row";
import { PortHistoryTimeline } from "@/components/port-history-timeline";
import { StopDialog } from "@/components/stop-dialog";
import {
  DEFAULT_COLUMN_SIZING,
  clampColumnWidth,
  loadColumnSizing,
  saveColumnSizing,
  totalColumnWidth,
} from "@/lib/column-sizing";
import { portHintsLabel } from "@/lib/port-hints";
import { filterPortProcesses } from "@/lib/port-filter";
import type { AppSettings, PortProcess, RowChangeKind, SystemKind } from "@/lib/types";
import {
  formatPorts,
  formatUptime,
  groupDirectory,
  isPinned,
  systemKindLabel,
} from "@/lib/types";
import { cn } from "@/lib/utils";

interface PortTableProps {
  processes: PortProcess[];
  search: string;
  settings: AppSettings;
  rowChanges: Map<number, RowChangeKind>;
  onRefresh: () => void;
  onRefreshPauseChange: (paused: boolean) => void;
  onTogglePinnedPath: (path: string) => void;
  onUseHttpsForLocalhostChange: (useHttps: boolean) => void;
}

function changeBadge(change: RowChangeKind | undefined) {
  if (!change) {
    return null;
  }

  return (
    <Badge
      variant="outline"
      className={cn(
        "ml-2 text-[10px] uppercase",
        change === "new" && "border-emerald-500/40 text-emerald-600 dark:text-emerald-400",
        change === "changed" && "border-amber-500/40 text-amber-600 dark:text-amber-400",
      )}
    >
      {change}
    </Badge>
  );
}

function kindBadgeVariant(kind: SystemKind): "apple" | "system" | "user" {
  return kind;
}

function stickyCellClass(position: "first" | "last" | "corner-left" | "corner-right") {
  const base =
    "bg-background group-hover:bg-[color-mix(in_oklch,var(--muted)_50%,var(--background))]";
  switch (position) {
    case "first":
      return cn(base, "sticky left-0 z-10");
    case "last":
      return cn(base, "sticky right-0 z-10");
    case "corner-left":
      return cn("bg-background", "sticky left-0 top-0 z-30");
    case "corner-right":
      return cn("bg-background", "sticky right-0 top-0 z-30");
  }
}

export function PortTable({
  processes,
  search,
  settings,
  rowChanges,
  onRefresh,
  onRefreshPauseChange,
  onTogglePinnedPath,
  onUseHttpsForLocalhostChange,
}: PortTableProps) {
  const [sorting, setSorting] = useState<SortingState>([{ id: "ports", desc: false }]);
  const [rowSelection, setRowSelection] = useState<RowSelectionState>({});
  const [columnSizing, setColumnSizing] = useState<ColumnSizingState>(loadColumnSizing);
  const tableRef = useRef<HTMLTableElement>(null);
  const colRefs = useRef<Record<string, HTMLTableColElement | null>>({});
  const isResizingRef = useRef(false);
  const dragRef = useRef<{
    columnId: string;
    startX: number;
    startWidth: number;
    minSize: number;
    maxSize: number;
    liveSizing: ColumnSizingState;
    rafId: number | null;
  } | null>(null);
  const [openMenuPid, setOpenMenuPid] = useState<number | null>(null);
  const [stopTargets, setStopTargets] = useState<PortProcess[]>([]);
  const [stopDialogTitle, setStopDialogTitle] = useState<string | undefined>();
  const [stopDialogDescription, setStopDialogDescription] = useState<
    string | undefined
  >();
  const [deleteTarget, setDeleteTarget] = useState<{
    process: PortProcess;
    mode: "trash" | "permanent";
  } | null>(null);
  const [historyPort, setHistoryPort] = useState<number | null>(null);

  const openStopDialog = useCallback(
    (targets: PortProcess[], title?: string, description?: string) => {
      setStopDialogTitle(title);
      setStopDialogDescription(description);
      setStopTargets(targets);
    },
    [],
  );

  useEffect(() => {
    onRefreshPauseChange(
      openMenuPid !== null ||
        stopTargets.length > 0 ||
        deleteTarget !== null ||
        historyPort !== null ||
        isResizingRef.current,
    );
  }, [openMenuPid, stopTargets.length, deleteTarget, historyPort, onRefreshPauseChange]);

  const applyLiveSizing = useCallback((sizing: ColumnSizingState) => {
    for (const [columnId, width] of Object.entries(sizing)) {
      colRefs.current[columnId]?.style.setProperty("width", `${width}px`);
    }
    if (tableRef.current) {
      tableRef.current.style.width = `${totalColumnWidth(sizing)}px`;
    }
  }, []);

  const handleResizePointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>, header: Header<PortProcess, unknown>) => {
      if (!header.column.getCanResize()) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();

      const handle = event.currentTarget;
      handle.setPointerCapture(event.pointerId);

      const columnId = header.column.id;
      const startWidth = header.getSize();
      const minSize = header.column.columnDef.minSize ?? 20;
      const maxSize = header.column.columnDef.maxSize ?? Number.MAX_SAFE_INTEGER;

      dragRef.current = {
        columnId,
        startX: event.clientX,
        startWidth,
        minSize,
        maxSize,
        liveSizing: { ...columnSizing },
        rafId: null,
      };
      isResizingRef.current = true;
      onRefreshPauseChange(true);
      document.body.style.userSelect = "none";
      document.body.style.cursor = "col-resize";

      const scheduleApply = (width: number) => {
        const drag = dragRef.current;
        if (!drag) {
          return;
        }

        drag.liveSizing = { ...drag.liveSizing, [columnId]: width };
        if (drag.rafId !== null) {
          return;
        }

        drag.rafId = requestAnimationFrame(() => {
          const active = dragRef.current;
          if (!active) {
            return;
          }
          active.rafId = null;
          applyLiveSizing(active.liveSizing);
        });
      };

      const finishResize = (clientX: number) => {
        const drag = dragRef.current;
        if (!drag) {
          return;
        }

        if (drag.rafId !== null) {
          cancelAnimationFrame(drag.rafId);
        }

        const newWidth = clampColumnWidth(
          drag.startWidth + (clientX - drag.startX),
          drag.minSize,
          drag.maxSize,
        );
        const nextSizing = { ...columnSizing, [columnId]: newWidth };

        dragRef.current = null;
        isResizingRef.current = false;
        document.body.style.userSelect = "";
        document.body.style.cursor = "";
        onRefreshPauseChange(
          openMenuPid !== null || stopTargets.length > 0 || deleteTarget !== null || historyPort !== null,
        );

        setColumnSizing(nextSizing);
        saveColumnSizing(nextSizing);
        applyLiveSizing(nextSizing);
      };

      const onPointerMove = (moveEvent: PointerEvent) => {
        if (!dragRef.current || moveEvent.pointerId !== event.pointerId) {
          return;
        }

        scheduleApply(
          clampColumnWidth(
            dragRef.current.startWidth + (moveEvent.clientX - dragRef.current.startX),
            dragRef.current.minSize,
            dragRef.current.maxSize,
          ),
        );
      };

      const onPointerUp = (upEvent: PointerEvent) => {
        if (upEvent.pointerId !== event.pointerId) {
          return;
        }

        handle.removeEventListener("pointermove", onPointerMove);
        handle.removeEventListener("pointerup", onPointerUp);
        handle.removeEventListener("pointercancel", onPointerUp);
        if (handle.hasPointerCapture(upEvent.pointerId)) {
          handle.releasePointerCapture(upEvent.pointerId);
        }
        finishResize(upEvent.clientX);
      };

      handle.addEventListener("pointermove", onPointerMove);
      handle.addEventListener("pointerup", onPointerUp);
      handle.addEventListener("pointercancel", onPointerUp);
    },
    [
      applyLiveSizing,
      columnSizing,
      deleteTarget,
      historyPort,
      onRefreshPauseChange,
      openMenuPid,
      stopTargets.length,
      deleteTarget,
    ],
  );

  const canStop = useCallback(
    (process: PortProcess) =>
      !process.is_system_service || settings.allowSystemProcessActions,
    [settings.allowSystemProcessActions],
  );

  const actionHandlers = useMemo<PortTableActionsHandlers>(
    () => ({
      canStop,
      openStopDialog,
      onTogglePinnedPath,
      onUseHttpsForLocalhostChange,
      setHistoryPort,
      setDeleteTarget,
    }),
    [canStop, openStopDialog, onTogglePinnedPath, onUseHttpsForLocalhostChange],
  );

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

  const openFolder = async (path: string) => {
    try {
      await invoke("open_in_finder", { path });
    } catch (err) {
      toast.error(String(err));
    }
  };

  const visibleProcesses = useMemo(
    () =>
      filterPortProcesses(
        processes,
        settings.hideSystemServices,
        settings.hideUserServices,
        search,
        settings.searchField,
      ),
    [
      processes,
      search,
      settings.hideSystemServices,
      settings.hideUserServices,
      settings.searchField,
    ],
  );

  const tableData = useMemo(() => {
    const sorted = [...visibleProcesses].sort((a, b) => {
      const aPinned = isPinned(a, settings.pinnedPaths);
      const bPinned = isPinned(b, settings.pinnedPaths);
      if (aPinned !== bPinned) {
        return aPinned ? -1 : 1;
      }

      if (settings.groupByDirectory) {
        const groupCompare = groupDirectory(a).localeCompare(groupDirectory(b));
        if (groupCompare !== 0) {
          return groupCompare;
        }
      }

      return (a.ports[0]?.port ?? 0) - (b.ports[0]?.port ?? 0);
    });

    return sorted;
  }, [visibleProcesses, settings.groupByDirectory, settings.pinnedPaths]);

  const columns = useMemo<ColumnDef<PortProcess>[]>(() => {
    const includeProtocol = settings.includeUdp;
    const allColumns: ColumnDef<PortProcess>[] = [
      {
        id: "select",
        header: ({ table }) => (
          <input
            type="checkbox"
            className="size-4 accent-primary"
            checked={table.getIsAllPageRowsSelected()}
            ref={(element) => {
              if (element) {
                element.indeterminate =
                  table.getIsSomePageRowsSelected() &&
                  !table.getIsAllPageRowsSelected();
              }
            }}
            onChange={table.getToggleAllPageRowsSelectedHandler()}
            aria-label="Select all visible processes"
          />
        ),
        size: DEFAULT_COLUMN_SIZING.select,
        minSize: 40,
        maxSize: 40,
        enableResizing: false,
        cell: ({ row }) => (
          <input
            type="checkbox"
            className="size-4 accent-primary"
            checked={row.getIsSelected()}
            disabled={!canStop(row.original)}
            onChange={row.getToggleSelectedHandler()}
            aria-label={`Select ${row.original.name}`}
          />
        ),
      },
      {
        id: "ports",
        accessorFn: (row) => row.ports[0]?.port ?? 0,
        header: "Port(s)",
        size: DEFAULT_COLUMN_SIZING.ports,
        minSize: 72,
        maxSize: 400,
        cell: ({ row }) => {
          const hint = portHintsLabel(row.original.ports);
          const portsText = formatPorts(row.original.ports, includeProtocol);
          return (
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="flex min-w-0 flex-col truncate font-mono text-sm">
                  <span className="flex items-center truncate">
                    <span className="truncate">{portsText}</span>
                    {changeBadge(rowChanges.get(row.original.pid))}
                  </span>
                  {hint && (
                    <span className="truncate text-[10px] text-muted-foreground">
                      {hint}
                    </span>
                  )}
                </span>
              </TooltipTrigger>
              {hint && (
                <TooltipContent side="bottom">{hint}</TooltipContent>
              )}
            </Tooltip>
          );
        },
      },
      {
        accessorKey: "name",
        id: "name",
        header: "Process",
        size: DEFAULT_COLUMN_SIZING.name,
        minSize: 72,
        maxSize: 240,
        cell: ({ row }) => (
          <span className="block truncate font-medium">{row.original.name}</span>
        ),
      },
      {
        accessorKey: "pid",
        id: "pid",
        header: "PID",
        size: DEFAULT_COLUMN_SIZING.pid,
        minSize: 56,
        maxSize: 120,
        cell: ({ row }) => (
          <span className="block truncate font-mono">{row.original.pid}</span>
        ),
      },
      {
        accessorKey: "user",
        header: "User",
        size: DEFAULT_COLUMN_SIZING.user,
        minSize: 72,
        maxSize: 160,
        cell: ({ row }) => (
          <span className="block truncate">{row.original.user}</span>
        ),
      },
      {
        id: "script",
        header: "Script / Command",
        size: DEFAULT_COLUMN_SIZING.script,
        minSize: 120,
        maxSize: 600,
        cell: ({ row }) => {
          const display =
            row.original.script_path ||
            row.original.command_line ||
            row.original.executable_path;
          return (
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="block truncate text-sm">{display}</span>
              </TooltipTrigger>
              <TooltipContent side="bottom" className="max-w-md">
                {row.original.command_line || display}
              </TooltipContent>
            </Tooltip>
          );
        },
      },
      {
        id: "directory",
        header: "Directory",
        size: DEFAULT_COLUMN_SIZING.directory,
        minSize: 120,
        maxSize: 500,
        cell: ({ row }) => {
          const cwd = row.original.working_directory;
          if (!cwd) return <span className="text-muted-foreground">—</span>;
          return (
            <Tooltip>
              <TooltipTrigger asChild>
                <button
                  type="button"
                  className="block w-full truncate text-left text-sm text-primary hover:underline"
                  onClick={() => void openFolder(cwd)}
                >
                  {cwd}
                </button>
              </TooltipTrigger>
              <TooltipContent side="bottom" className="max-w-md">
                {cwd}
              </TooltipContent>
            </Tooltip>
          );
        },
      },
      {
        id: "uptime",
        accessorKey: "uptime_seconds",
        header: "Uptime",
        size: DEFAULT_COLUMN_SIZING.uptime,
        minSize: 72,
        maxSize: 160,
        cell: ({ row }) => (
          <span className="block truncate font-mono text-sm">
            {formatUptime(row.original.uptime_seconds)}
          </span>
        ),
      },
      {
        id: "type",
        header: "Type",
        size: DEFAULT_COLUMN_SIZING.type,
        minSize: 96,
        maxSize: 180,
        enableResizing: false,
        cell: ({ row }) => (
          <Badge variant={kindBadgeVariant(row.original.system_kind)}>
            {systemKindLabel(row.original.system_kind)}
          </Badge>
        ),
      },
      {
        id: "actions",
        header: "",
        size: DEFAULT_COLUMN_SIZING.actions,
        minSize: 52,
        maxSize: 52,
        enableResizing: false,
        cell: ({ row }) => (
          <PortTableActionsCell
            process={row.original}
            settings={actionSettings}
            handlers={actionHandlers}
          />
        ),
      },
    ];

    return allColumns;
  }, [
    actionHandlers,
    actionSettings,
    canStop,
    rowChanges,
    settings.includeUdp,
  ]);

  const table = useReactTable({
    data: tableData,
    columns,
    state: { sorting, columnSizing, rowSelection },
    onSortingChange: setSorting,
    onColumnSizingChange: setColumnSizing,
    onRowSelectionChange: setRowSelection,
    enableRowSelection: (row) => canStop(row.original),
    getRowId: (row) => String(row.pid),
    columnResizeMode: "onEnd",
    enableColumnResizing: true,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  const selectedProcesses = useMemo(
    () => table.getSelectedRowModel().rows.map((row) => row.original),
    [table, rowSelection, tableData],
  );

  const stoppableUserProcesses = useMemo(
    () =>
      table
        .getRowModel()
        .rows.map((row) => row.original)
        .filter((process) => canStop(process) && !process.is_system_service),
    [table, tableData, canStop],
  );

  const columnCount = columns.length;
  const tableWidth = totalColumnWidth(columnSizing);

  type TableRowItem =
    | { kind: "group"; id: string; label: string }
    | { kind: "data"; id: string; row: Row<PortProcess> };

  const tableRows = useMemo(() => {
    const rows = table.getRowModel().rows;
    if (rows.length === 0) {
      return [] as TableRowItem[];
    }

    const items: TableRowItem[] = [];
    let lastGroup: string | null = null;
    let pinnedHeaderShown = false;

    for (const row of rows) {
      const pinned = isPinned(row.original, settings.pinnedPaths);

      if (pinned && !pinnedHeaderShown) {
        pinnedHeaderShown = true;
        items.push({ kind: "group", id: "group-pinned", label: "Pinned" });
      }

      if (settings.groupByDirectory) {
        const group = groupDirectory(row.original);
        if (group !== lastGroup) {
          lastGroup = group;
          items.push({ kind: "group", id: `group-${group}`, label: group });
        }
      }

      items.push({ kind: "data", id: row.id, row });
    }

    return items;
  }, [table, tableData, settings.groupByDirectory, settings.pinnedPaths]);

  return (
    <OpenMenuProvider openMenuPid={openMenuPid} setOpenMenuPid={setOpenMenuPid}>
    <TooltipProvider>
      <div className="flex h-full min-h-0 flex-col">
      {selectedProcesses.length > 0 && (
        <div className="mb-2 flex flex-wrap items-center gap-2 rounded-md border bg-muted/30 px-3 py-2">
          <span className="text-sm text-muted-foreground">
            {selectedProcesses.length} selected
          </span>
          <Button
            size="sm"
            variant="destructive"
            onClick={() =>
              openStopDialog(
                selectedProcesses,
                `Stop ${selectedProcesses.length} selected processes?`,
              )
            }
          >
            Stop selected
          </Button>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button size="sm" variant="outline">
                Batch actions
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start">
              <DropdownMenuItem
                disabled={stoppableUserProcesses.length === 0}
                onClick={() =>
                  openStopDialog(
                    stoppableUserProcesses,
                    `Stop all ${stoppableUserProcesses.length} visible user processes?`,
                    "This stops every visible user process in the current table view.",
                  )
                }
              >
                Stop all visible user processes
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => setRowSelection({})}
          >
            Clear selection
          </Button>
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-auto rounded-md border">
        <table
          ref={tableRef}
          className="caption-bottom text-sm"
          style={{
            width: tableWidth,
            minWidth: "100%",
            tableLayout: "fixed",
          }}
        >
          <colgroup>
            {table.getAllLeafColumns().map((column) => (
              <col
                key={column.id}
                ref={(element) => {
                  colRefs.current[column.id] = element;
                }}
                style={{ width: column.getSize() }}
              />
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
                        : flexRender(header.column.columnDef.header, header.getContext())}
                      {header.column.getCanResize() && (
                        <div
                          onPointerDown={(event) => handleResizePointerDown(event, header)}
                          onDoubleClick={() => header.column.resetSize()}
                          className="group/resize absolute top-0 -right-1 z-40 h-full w-2 cursor-col-resize touch-none select-none"
                        >
                          <div
                            className={cn(
                              "absolute top-0 left-1/2 h-full w-px -translate-x-1/2 bg-border opacity-0 transition-opacity group-hover/resize:opacity-100",
                              header.column.getIsResizing() && "bg-primary opacity-100",
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
          <tbody className="[&_tr:last-child]:border-0">
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
                    key={item.id}
                    row={item.row}
                    change={rowChanges.get(item.row.original.pid)}
                    columnCount={columnCount}
                  />
                );
              })
            ) : (
              <tr className="border-b">
                <td colSpan={columnCount} className="h-24 p-2 text-center align-middle">
                  {processes.length > 0 ? (
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
      </div>
      </div>

      <StopDialog
        processes={stopTargets}
        open={stopTargets.length > 0}
        onOpenChange={(open) => {
          if (!open) {
            setStopTargets([]);
            setStopDialogTitle(undefined);
            setStopDialogDescription(undefined);
            setRowSelection({});
          }
        }}
        title={stopDialogTitle}
        description={stopDialogDescription}
        requireDoubleConfirm={
          stopTargets.some((process) => process.is_system_service) &&
          settings.allowSystemProcessActions
        }
        onStopped={onRefresh}
      />

      <DeleteDialog
        target={deleteTarget}
        open={deleteTarget !== null}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        onComplete={onRefresh}
      />

      <Dialog
        open={historyPort !== null}
        onOpenChange={(open) => !open && setHistoryPort(null)}
      >
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>
              Port {historyPort} history
            </DialogTitle>
            <DialogDescription>
              Occupied and freed events recorded during scans.
            </DialogDescription>
          </DialogHeader>
          {historyPort !== null && <PortHistoryTimeline port={historyPort} />}
        </DialogContent>
      </Dialog>
    </TooltipProvider>
    </OpenMenuProvider>
  );
}
