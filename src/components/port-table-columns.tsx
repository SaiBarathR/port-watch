import { invoke } from "@tauri-apps/api/core";
import type { ColumnDef, Table } from "@tanstack/react-table";
import { GlobeIcon, LockIcon, NetworkIcon } from "lucide-react";
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
  folderLabel,
  heldPorts,
  portsThatFit,
  protocolTag,
  reachLabel,
  type HeldPort,
} from "@/lib/ports";
import {
  pinPath,
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
  /** How wide the port column is, in px. */
  portsColumnWidth: number;
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

const REACH_ICON = {
  everyone: GlobeIcon,
  "one-address": NetworkIcon,
  "this-machine": LockIcon,
} as const;

// The port is what a row is about, so it leads and is the largest thing in
// it. The glyph says who can reach it, which is the fact worth a glance:
// a dev server open to the whole network is rarely meant to be.
function PortNumber({ held }: { held: HeldPort }) {
  const Icon = REACH_ICON[held.reach];
  const protocol = protocolTag(held);
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span className="inline-flex shrink-0 items-center gap-1 font-mono text-[15px] leading-5 font-semibold tabular-nums">
          {held.port}
          {protocol && (
            <span className="text-[10px] font-normal text-muted-foreground">
              {protocol}
            </span>
          )}
          <Icon
            aria-hidden
            className={cn(
              "size-3",
              held.reach === "this-machine"
                ? "text-muted-foreground/60"
                : "text-amber-600 dark:text-amber-400",
            )}
          />
          <span className="sr-only">{reachLabel(held)}</span>
        </span>
      </TooltipTrigger>
      <TooltipContent side="bottom">
        <p>{reachLabel(held)}</p>
        <p className="font-mono text-muted-foreground">
          {held.addresses
            .map((address) => `${address}:${held.port}`)
            .join(", ")}{" "}
          {held.protocols.join(" and ")}
        </p>
      </TooltipContent>
    </Tooltip>
  );
}

// What is in a port cell beside the ports: its padding, and the badge of a
// row that just changed.
const CELL_PADDING = 16;
const CHANGE_BADGE = 72;

function portText(held: HeldPort): string {
  const protocol = protocolTag(held);
  return protocol ? `${held.port}/${protocol}` : String(held.port);
}

function PortsCell({
  process,
  change,
  width,
}: {
  process: PortProcess;
  change: RowChangeKind | undefined;
  /** The column's width: as many ports are spelled out as fit in it. */
  width: number;
}) {
  const ports = heldPorts(process.ports);
  const shown = portsThatFit(
    ports,
    width - CELL_PADDING - (change ? CHANGE_BADGE : 0),
  );
  const rest = ports.slice(shown);
  // The labels are guesses from the port number alone ("5000: Flask"), good
  // enough for a dev server and wrong for what the system runs there.
  const hint = process.is_system_service
    ? undefined
    : portHintsLabel(process.ports);

  return (
    <span className="flex min-w-0 flex-col">
      <span className="flex min-w-0 items-center gap-2">
        {ports.slice(0, shown).map((held) => (
          <PortNumber key={held.port} held={held} />
        ))}
        {rest.length > 0 && (
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                className="shrink-0 rounded-sm text-xs text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
                aria-label={`${rest.length} more port${rest.length === 1 ? "" : "s"}: ${rest.map(portText).join(", ")}`}
              >
                +{rest.length}
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom" className="font-mono">
              {rest.map(portText).join(", ")}
            </TooltipContent>
          </Tooltip>
        )}
        {changeBadge(change)}
      </span>
      {hint && (
        <span className="truncate text-[10px] text-muted-foreground">
          {hint}
        </span>
      )}
    </span>
  );
}

// The folder's own name is what tells one project from another; where it
// sits is context. The root says nothing about a process, so it is a dash.
function ProjectCell({ process }: { process: PortProcess }) {
  const folder = pinPath(process);
  const label = folderLabel(folder);
  if (!label) {
    return <span className="text-muted-foreground">—</span>;
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          className="flex w-full min-w-0 items-baseline gap-1.5 text-left text-sm hover:underline"
          onClick={() => void openFolder(folder)}
        >
          <span className="shrink-0 font-medium">{label.name}</span>
          <span className="truncate text-xs text-muted-foreground">
            {label.parent}
          </span>
        </button>
      </TooltipTrigger>
      <TooltipContent side="bottom" className="max-w-md">
        {folder}
      </TooltipContent>
    </Tooltip>
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
    header: "Port",
    size: DEFAULT_COLUMN_SIZING.ports,
    minSize: 72,
    maxSize: 400,
    cell: ({ row, table }) => (
      <PortsCell
        process={row.original}
        change={metaOf(table).rowChanges.get(row.original.id)}
        width={metaOf(table).portsColumnWidth}
      />
    ),
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
    header: "Project",
    size: DEFAULT_COLUMN_SIZING.directory,
    minSize: 120,
    maxSize: 500,
    cell: ({ row }) => <ProjectCell process={row.original} />,
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
