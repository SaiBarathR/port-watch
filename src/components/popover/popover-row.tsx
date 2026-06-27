import { invoke } from "@tauri-apps/api/core";
import {
  ExternalLinkIcon,
  FolderOpenIcon,
  OctagonIcon,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { PortProcess, RowChangeKind } from "@/lib/types";
import {
  groupDirectory,
  localhostUrl,
  primaryPort,
} from "@/lib/types";
import { cn } from "@/lib/utils";

interface PopoverRowProps {
  process: PortProcess;
  change?: RowChangeKind;
  canStop: boolean;
  useHttpsForLocalhost: boolean;
  onStop: (process: PortProcess) => void;
}

function changeDotClass(change: RowChangeKind | undefined): string {
  switch (change) {
    case "new":
      return "bg-emerald-500";
    case "changed":
      return "bg-amber-500";
    default:
      return "bg-transparent";
  }
}

export function PopoverRow({
  process,
  change,
  canStop,
  useHttpsForLocalhost,
  onStop,
}: PopoverRowProps) {
  const port = primaryPort(process);
  const directory = groupDirectory(process);
  const hasDirectory =
    directory !== "Unknown" &&
    (process.working_directory || process.project_root);

  const openFolder = async () => {
    const path = process.working_directory || process.project_root;
    if (!path) {
      toast.error("No folder available");
      return;
    }
    try {
      await invoke("open_in_finder", { path });
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
      await invoke("open_url", { url: localhostUrl(port, useHttpsForLocalhost) });
    } catch (err) {
      toast.error(String(err));
    }
  };

  return (
    <div
      className={cn(
        "flex items-center gap-2 px-3 py-2 transition-colors hover:bg-muted/40",
        change === "new" && "bg-emerald-500/5",
        change === "changed" && "bg-amber-500/5",
      )}
    >
      <span
        className={cn(
          "size-1.5 shrink-0 rounded-full",
          changeDotClass(change),
        )}
        aria-hidden
      />

      <div className="min-w-0 flex-1">
        <div className="flex min-w-0 items-baseline gap-2">
          <span className="shrink-0 rounded bg-muted px-1.5 py-0.5 font-mono text-xs font-medium">
            :{port ?? "—"}
          </span>
          <span className="truncate text-sm font-medium">{process.name}</span>
        </div>
        {hasDirectory && (
          <Tooltip>
            <TooltipTrigger asChild>
              <p className="mt-0.5 truncate text-xs text-muted-foreground">
                {directory}
              </p>
            </TooltipTrigger>
            <TooltipContent side="bottom" className="max-w-xs">
              {directory}
            </TooltipContent>
          </Tooltip>
        )}
      </div>

      <div className="flex shrink-0 items-center gap-0.5">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="size-7"
              disabled={!canStop}
              onClick={() => onStop(process)}
              aria-label={`Stop ${process.name}`}
            >
              <OctagonIcon className="size-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Stop</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="size-7"
              disabled={port === null}
              onClick={() => void openInBrowser()}
              aria-label="Open in browser"
            >
              <ExternalLinkIcon className="size-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>
            Open in browser
            {useHttpsForLocalhost ? " (HTTPS)" : ""}
          </TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="size-7"
              disabled={!process.working_directory && !process.project_root}
              onClick={() => void openFolder()}
              aria-label="Open folder"
            >
              <FolderOpenIcon className="size-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Open folder</TooltipContent>
        </Tooltip>
      </div>
    </div>
  );
}
