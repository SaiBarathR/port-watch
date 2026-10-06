import type { PortProcess, PreferredEditor } from "@/lib/types";
import { isPinned, pinPath, primaryPort } from "@/lib/types";

export type RowActionId =
  | "open-browser"
  | "copy-url"
  | "open-editor"
  | "open-terminal"
  | "reveal"
  | "copy-path"
  | "copy-command"
  | "pin"
  | "watch"
  | "history"
  | "stop"
  | "free-port"
  | "trash"
  | "delete";

export interface RowAction {
  id: RowActionId;
  label: string;
  /** The keys that do the same from a focused row. */
  shortcut?: string;
  destructive?: boolean;
  /** Why this row cannot have it done. The item is disabled and says so. */
  disabledReason?: string;
}

export interface RowActionContext {
  platform: "macos" | "linux" | "windows" | "unknown";
  preferredEditor: PreferredEditor;
  pinnedPaths: string[];
  watchedPorts: number[];
  allowSystemProcessActions: boolean;
  /** Whether other processes hold this row's port too. */
  portIsShared: boolean;
  /** While the key that shows the alternate items is held (⌥, or Shift). */
  alternate: boolean;
}

const KEYS = {
  macos: {
    "open-browser": "↵",
    "open-editor": "⌘O",
    "copy-url": "⇧⌘C",
    stop: "⌘⌫",
  },
  other: {
    "open-browser": "Enter",
    "open-editor": "Ctrl+O",
    "copy-url": "Ctrl+Shift+C",
    stop: "Ctrl+Backspace",
  },
} as const;

/** Why a process cannot be stopped from the app, if it cannot. */
export function stopBlockedReason(
  process: PortProcess,
  allowSystemProcessActions: boolean,
): string | undefined {
  if (process.pid === 0) {
    return "The process behind this listener is not visible to you.";
  }
  if (process.is_system_service && !allowSystemProcessActions) {
    return "System services are locked. Settings can allow actions on them.";
  }
  return undefined;
}

/**
 * A row's menu, group by group, in the order a row gets used: open what it
 * serves, work on its project, keep track of it, stop it, delete it. The
 * dropdown and the right-click menu both draw this.
 */
export function rowActions(
  process: PortProcess,
  context: RowActionContext,
): RowAction[][] {
  const keys = context.platform === "macos" ? KEYS.macos : KEYS.other;
  const port = primaryPort(process);
  // One folder for every action that needs one: the project if one was
  // found, else where the process runs.
  const folder = pinPath(process);
  const noFolder = folder ? undefined : "No folder is known for this process.";
  const stopBlocked = stopBlockedReason(
    process,
    context.allowSystemProcessActions,
  );

  const open: RowAction[] =
    port === null
      ? []
      : [
          {
            id: "open-browser",
            label: "Open in Browser",
            shortcut: keys["open-browser"],
          },
          { id: "copy-url", label: "Copy URL", shortcut: keys["copy-url"] },
        ];

  const project: RowAction[] = [
    {
      id: "open-editor",
      label: `Open in ${context.preferredEditor === "code" ? "VS Code" : "Cursor"}`,
      shortcut: keys["open-editor"],
      disabledReason: noFolder,
    },
    {
      id: "open-terminal",
      label: "Open in Terminal",
      disabledReason: noFolder,
    },
    {
      id: "reveal",
      label:
        context.platform === "macos"
          ? "Reveal in Finder"
          : context.platform === "windows"
            ? "Show in Explorer"
            : "Show in File Manager",
      disabledReason: noFolder,
    },
    { id: "copy-path", label: "Copy Folder Path", disabledReason: noFolder },
    // The row cuts a long command short, and shows it whole only in a
    // tooltip the keyboard cannot open.
    {
      id: "copy-command",
      label: "Copy Command",
      disabledReason: process.command_line
        ? undefined
        : "This process's command line could not be read.",
    },
  ];

  const track: RowAction[] = [
    {
      id: "pin",
      label: isPinned(process, context.pinnedPaths)
        ? "Unpin Project"
        : "Pin Project",
      disabledReason: noFolder,
    },
  ];
  if (port !== null) {
    track.push(
      {
        id: "watch",
        label: context.watchedPorts.includes(port)
          ? `Stop Watching Port ${port}`
          : `Watch Port ${port}`,
      },
      { id: "history", label: `Port ${port} History` },
    );
  }

  const stop: RowAction[] = [
    {
      id: "stop",
      label: "Stop…",
      shortcut: keys.stop,
      disabledReason: stopBlocked,
    },
  ];
  // Only worth offering when it differs from Stop.
  if (port !== null && context.portIsShared) {
    stop.push({
      id: "free-port",
      label: `Free Port ${port}…`,
      disabledReason: stopBlocked,
    });
  }

  // The folder is removed after the process is stopped, so whatever blocks
  // the stop blocks this too.
  const deleteBlocked = noFolder ?? stopBlocked ?? process.delete_blocked;
  const remove: RowAction[] = [
    context.alternate
      ? {
          id: "delete",
          label: "Delete Permanently…",
          destructive: true,
          disabledReason: deleteBlocked ?? undefined,
        }
      : {
          id: "trash",
          label:
            context.platform === "windows"
              ? "Move to Recycle Bin…"
              : "Move to Trash…",
          destructive: true,
          disabledReason: deleteBlocked ?? undefined,
        },
  ];

  return [open, project, track, stop, remove].filter(
    (group) => group.length > 0,
  );
}
