import { invoke } from "@tauri-apps/api/core";
import { ArrowRightIcon, RefreshCwIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface PopoverFooterProps {
  userCount: number;
  loading: boolean;
  onRefresh: () => void;
}

export function PopoverFooter({
  userCount,
  loading,
  onRefresh,
}: PopoverFooterProps) {
  const openFullWindow = async () => {
    try {
      await invoke("show_full_window_command");
    } catch (err) {
      console.error(err);
    }
  };

  return (
    <div className="shrink-0 border-t px-3 py-2">
      <div className="flex items-center justify-between gap-2 text-xs text-muted-foreground">
        <span>
          {userCount} listener{userCount === 1 ? "" : "s"}
        </span>
        <Button
          variant="ghost"
          size="sm"
          className="h-7 gap-1 px-2 text-xs"
          onClick={onRefresh}
          disabled={loading}
        >
          <RefreshCwIcon className={cn("size-3", loading && "animate-spin")} />
          Refresh
        </Button>
      </div>
      <button
        type="button"
        className="mt-1 flex w-full items-center justify-center gap-1 rounded-md py-1.5 text-xs font-medium text-primary hover:bg-muted/50"
        onClick={() => void openFullWindow()}
      >
        Open Full Window
        <ArrowRightIcon className="size-3" />
      </button>
    </div>
  );
}
