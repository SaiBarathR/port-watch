import { createContext, memo, useContext, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  CopyIcon,
  ExternalLinkIcon,
  FolderOpenIcon,
  HistoryIcon,
  LinkIcon,
  MoreHorizontalIcon,
  OctagonIcon,
  PinIcon,
  PinOffIcon,
  TerminalIcon,
  Trash2Icon,
  TrashIcon,
  CodeIcon,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { AppSettings, PortProcess } from "@/lib/types";
import {
  isPinned,
  localhostUrl,
  pinPath,
  primaryPath,
  primaryPort,
} from "@/lib/types";

export interface PortTableActionsHandlers {
  canStop: (process: PortProcess) => boolean;
  openStopDialog: (
    targets: PortProcess[],
    title?: string,
    description?: string,
  ) => void;
  onTogglePinnedPath: (path: string) => void;
  onUseHttpsForLocalhostChange: (useHttps: boolean) => void;
  setHistoryPort: (port: number) => void;
  setDeleteTarget: (
    target: { process: PortProcess; mode: "trash" | "permanent" } | null,
  ) => void;
}

interface OpenMenuContextValue {
  openMenuPid: number | null;
  setOpenMenuPid: (pid: number | null) => void;
}

export const OpenMenuContext = createContext<OpenMenuContextValue | null>(null);

export function OpenMenuProvider({
  openMenuPid,
  setOpenMenuPid,
  children,
}: OpenMenuContextValue & { children: ReactNode }) {
  return (
    <OpenMenuContext.Provider value={{ openMenuPid, setOpenMenuPid }}>
      {children}
    </OpenMenuContext.Provider>
  );
}

interface PortTableActionsCellProps {
  process: PortProcess;
  settings: Pick<
    AppSettings,
    "pinnedPaths" | "preferredEditor" | "useHttpsForLocalhost"
  >;
  handlers: PortTableActionsHandlers;
}

export const PortTableActionsCell = memo(function PortTableActionsCell({
  process,
  settings,
  handlers,
}: PortTableActionsCellProps) {
  const menu = useContext(OpenMenuContext);
  if (!menu) {
    return null;
  }

  const { openMenuPid, setOpenMenuPid } = menu;
  const isOpen = openMenuPid === process.pid;
  const folderPath = pinPath(process) || process.working_directory;
  const editorPath = process.project_root || process.working_directory;
  const port = primaryPort(process);
  const pinnedPath = pinPath(process);
  const pinned = isPinned(process, settings.pinnedPaths);

  const openFolder = async (path: string) => {
    try {
      await invoke("open_in_finder", { path });
    } catch (err) {
      toast.error(String(err));
    }
  };

  const copyPath = async () => {
    const path = primaryPath(process);
    try {
      await navigator.clipboard.writeText(path);
      toast.success("Path copied to clipboard");
    } catch (err) {
      toast.error(String(err));
    }
  };

  const copyUrl = async () => {
    if (port === null) {
      toast.error("No port available");
      return;
    }
    const url = localhostUrl(port, settings.useHttpsForLocalhost);
    try {
      await navigator.clipboard.writeText(url);
      toast.success("URL copied to clipboard");
    } catch (err) {
      toast.error(String(err));
    }
  };

  const openInBrowser = async () => {
    if (port === null) {
      toast.error("No port available");
      return;
    }
    try {
      await invoke("open_url", {
        url: localhostUrl(port, settings.useHttpsForLocalhost),
      });
    } catch (err) {
      toast.error(String(err));
    }
  };

  const openInTerminal = async (cwd: string) => {
    if (!cwd) {
      toast.error("No working directory available");
      return;
    }
    try {
      await invoke("open_in_terminal", { cwd });
    } catch (err) {
      toast.error(String(err));
    }
  };

  const openInEditor = async (cwd: string) => {
    if (!cwd) {
      toast.error("No working directory available");
      return;
    }
    try {
      await invoke("open_in_editor", {
        cwd,
        editor: settings.preferredEditor,
      });
    } catch (err) {
      toast.error(String(err));
    }
  };

  return (
    <DropdownMenu
      open={isOpen}
      onOpenChange={(open) => {
        setOpenMenuPid(open ? process.pid : null);
      }}
      modal={false}
    >
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon">
          <MoreHorizontalIcon />
          <span className="sr-only">Actions</span>
        </Button>
      </DropdownMenuTrigger>
      {isOpen && (
        <DropdownMenuContent align="end">
          <DropdownMenuGroup>
            {port !== null && (
              <DropdownMenuItem
                disabled={!handlers.canStop(process)}
                onClick={() => {
                  setOpenMenuPid(null);
                  handlers.openStopDialog(
                    [process],
                    `Free port ${port}?`,
                    `Stop ${process.name} (PID ${process.pid}) to free port ${port}.`,
                  );
                }}
              >
                <OctagonIcon data-icon="inline-start" />
                Free port {port}
              </DropdownMenuItem>
            )}
            <DropdownMenuItem
              disabled={!handlers.canStop(process)}
              onClick={() => {
                setOpenMenuPid(null);
                handlers.openStopDialog([process]);
              }}
            >
              <OctagonIcon data-icon="inline-start" />
              Stop
            </DropdownMenuItem>
            {port !== null && (
              <DropdownMenuItem
                onClick={() => {
                  setOpenMenuPid(null);
                  handlers.setHistoryPort(port);
                }}
              >
                <HistoryIcon data-icon="inline-start" />
                Port {port} history
              </DropdownMenuItem>
            )}
            <DropdownMenuItem
              disabled={!folderPath}
              onClick={() => void openFolder(folderPath)}
            >
              <FolderOpenIcon data-icon="inline-start" />
              Reveal in file manager
            </DropdownMenuItem>
            {port !== null && (
              <>
                <DropdownMenuCheckboxItem
                  checked={settings.useHttpsForLocalhost}
                  onCheckedChange={handlers.onUseHttpsForLocalhostChange}
                  onSelect={(event) => event.preventDefault()}
                >
                  Use HTTPS
                </DropdownMenuCheckboxItem>
                <DropdownMenuItem onClick={() => void openInBrowser()}>
                  <ExternalLinkIcon data-icon="inline-start" />
                  Open in Browser
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => void copyUrl()}>
                  <LinkIcon data-icon="inline-start" />
                  Copy URL
                </DropdownMenuItem>
              </>
            )}
            <DropdownMenuItem onClick={() => void copyPath()}>
              <CopyIcon data-icon="inline-start" />
              Copy Path
            </DropdownMenuItem>
            <DropdownMenuItem
              disabled={!editorPath}
              onClick={() => void openInTerminal(editorPath)}
            >
              <TerminalIcon data-icon="inline-start" />
              Open in Terminal
            </DropdownMenuItem>
            <DropdownMenuItem
              disabled={!editorPath}
              onClick={() => void openInEditor(editorPath)}
            >
              <CodeIcon data-icon="inline-start" />
              Open in Editor
            </DropdownMenuItem>
            {pinnedPath && (
              <DropdownMenuItem
                onClick={() => handlers.onTogglePinnedPath(pinnedPath)}
              >
                {pinned ? (
                  <PinOffIcon data-icon="inline-start" />
                ) : (
                  <PinIcon data-icon="inline-start" />
                )}
                {pinned ? "Unpin project" : "Pin project"}
              </DropdownMenuItem>
            )}
          </DropdownMenuGroup>
          <DropdownMenuSeparator />
          <DropdownMenuGroup>
            <DropdownMenuItem
              variant="destructive"
              disabled={!folderPath || !handlers.canStop(process)}
              onClick={() => {
                setOpenMenuPid(null);
                handlers.setDeleteTarget({ process, mode: "trash" });
              }}
            >
              <TrashIcon data-icon="inline-start" />
              Move to Trash
            </DropdownMenuItem>
            <DropdownMenuItem
              variant="destructive"
              disabled={!folderPath || !handlers.canStop(process)}
              onClick={() => {
                setOpenMenuPid(null);
                handlers.setDeleteTarget({ process, mode: "permanent" });
              }}
            >
              <Trash2Icon data-icon="inline-start" />
              Delete Permanently
            </DropdownMenuItem>
          </DropdownMenuGroup>
        </DropdownMenuContent>
      )}
    </DropdownMenu>
  );
});
