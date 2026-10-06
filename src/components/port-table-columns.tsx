import { invoke } from "@tauri-apps/api/core";
import type { ColumnDef, Table } from "@tanstack/react-table";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { PortTableActionsCell } from "@/components/port-table-actions-cell";
import { Uptime } from "@/components/uptime";
import { DEFAULT_COLUMN_SIZING } from "@/lib/column-sizing";
import { portHintsLabel } from "@/lib/port-hints";
import {
  formatPorts,
  systemKindLabel,
  type AppSettings,
  type PortProcess,
  type RowChangeKind,
} from "@/lib/types";
import { cn } from "@/lib/utils";

// What the cell renderers need besides their row. It reaches them through the
// table's `meta` instead of closures, so the column definitions never change.
// flexRender mounts each cell renderer as a component: a new function would
// remount the cell, and an open row menu would lose its keyboard focus.
export interface PortTableMeta {
  includeUdp: boolean;
  rowChanges: Map<string, RowChangeKind>;
  canStop: (process: PortProcess) => boolean;
  actionSettings: Pick<
    AppSettings,
    "pinnedPaths" | "preferredEditor" | "useHttpsForLocalhost"
  >;
}

function metaOf(table: Table<PortProcess>): PortTableMeta {
  return table.options.meta as PortTableMeta;
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
        change === "new" &&
          "border-emerald-500/40 text-emerald-600 dark:text-emerald-400",
        change === "changed" &&
          "border-amber-500/40 text-amber-600 dark:text-amber-400",
      )}
    >
      {change}
    </Badge>
  );
}

async function openFolder(path: string) {
  try {
    await invoke("open_in_finder", { path });
  } catch (err) {
    toast.error(String(err));
  }
}

export const columns: ColumnDef<PortProcess>[] = [
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
    cell: ({ row, table }) => (
      <input
        type="checkbox"
        className="size-4 accent-primary"
        checked={row.getIsSelected()}
        disabled={!metaOf(table).canStop(row.original)}
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
    cell: ({ row, table }) => {
      const { includeUdp, rowChanges } = metaOf(table);
      const hint = portHintsLabel(row.original.ports);
      const portsText = formatPorts(row.original.ports, includeUdp);
      return (
        <Tooltip>
          <TooltipTrigger asChild>
            <span className="flex min-w-0 flex-col truncate font-mono text-sm">
              <span className="flex items-center truncate">
                <span className="truncate">{portsText}</span>
                {changeBadge(rowChanges.get(row.original.id))}
              </span>
              {hint && (
                <span className="truncate text-[10px] text-muted-foreground">
                  {hint}
                </span>
              )}
            </span>
          </TooltipTrigger>
          {hint && <TooltipContent side="bottom">{hint}</TooltipContent>}
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
    accessorKey: "started_at",
    header: "Uptime",
    size: DEFAULT_COLUMN_SIZING.uptime,
    minSize: 72,
    maxSize: 160,
    cell: ({ row }) => (
      <span className="block truncate font-mono text-sm">
        <Uptime startedAt={row.original.started_at} />
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
      <Badge variant={row.original.system_kind}>
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
    cell: ({ row, table }) => (
      <PortTableActionsCell
        process={row.original}
        settings={metaOf(table).actionSettings}
      />
    ),
  },
];
