import { useCallback, useDeferredValue, useEffect, useMemo, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AlertCircleIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { PopoverFooter } from "@/components/popover/popover-footer";
import { PopoverHeader } from "@/components/popover/popover-header";
import { PopoverList } from "@/components/popover/popover-list";
import { StopDialog } from "@/components/stop-dialog";
import { usePortScan } from "@/hooks/use-port-scan";
import { useLiquidGlass } from "@/hooks/use-liquid-glass";
import { getStoredTheme, resolveTheme } from "@/hooks/use-theme";
import type { PortProcess } from "@/lib/types";
import { cn } from "@/lib/utils";
import { processHasPort, primaryPort } from "@/lib/types";

function matchesPopoverSearch(process: PortProcess, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) {
    return true;
  }

  const portNum = Number.parseInt(query.trim(), 10);
  if (
    Number.isInteger(portNum) &&
    portNum >= 1 &&
    portNum <= 65535 &&
    String(portNum) === query.trim()
  ) {
    return processHasPort(process, portNum);
  }

  return (
    process.name.toLowerCase().includes(q) ||
    process.ports.some((binding) => String(binding.port).includes(q))
  );
}

export function PopoverApp() {
  const {
    allProcesses,
    loading,
    refreshing,
    error,
    refresh,
    settings,
    rowChanges,
    userCount,
  } = usePortScan();

  const [search, setSearch] = useState("");
  const deferredSearch = useDeferredValue(search);
  const [stopTarget, setStopTarget] = useState<PortProcess | null>(null);

  useEffect(() => {
    document.documentElement.classList.add("popover-window");
    document.body.classList.add("bg-transparent");

    return () => {
      document.documentElement.classList.remove("popover-window");
      document.body.classList.remove("bg-transparent");
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !stopTarget) {
        void getCurrentWindow().hide();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [stopTarget]);

  useLiquidGlass(settings.liquidGlass, "popover", resolveTheme(getStoredTheme()), {
    translucency: settings.glassTranslucency,
    blur: settings.glassBlur,
    tint: settings.glassTint,
  });

  const userListeners = useMemo(
    () => allProcesses.filter((process) => !process.is_system_service),
    [allProcesses],
  );

  const displayProcesses = useMemo(
    () =>
      userListeners.filter((process) =>
        matchesPopoverSearch(process, deferredSearch),
      ),
    [userListeners, deferredSearch],
  );

  const exactPortQuery = useMemo(() => {
    const trimmed = deferredSearch.trim();
    const port = Number.parseInt(trimmed, 10);
    if (
      !Number.isInteger(port) ||
      port < 1 ||
      port > 65535 ||
      String(port) !== trimmed
    ) {
      return null;
    }
    return port;
  }, [deferredSearch]);

  const portLookupEmpty =
    exactPortQuery !== null &&
    !loading &&
    !userListeners.some((process) => processHasPort(process, exactPortQuery));

  const handleStop = useCallback((process: PortProcess) => {
    setStopTarget(process);
  }, []);

  return (
    <div className="flex h-screen flex-col p-2">
      <div
        className={cn(
          "flex h-full flex-col overflow-hidden rounded-xl border text-sm shadow-lg",
          settings.liquidGlass
            ? "glass-window border-[var(--glass-border)]"
            : "bg-background",
        )}
      >
        <PopoverHeader search={search} onSearchChange={setSearch} />

        {error && (
          <Alert variant="destructive" className="mx-3 mt-2 shrink-0 py-2">
            <AlertCircleIcon className="size-4" />
            <AlertTitle className="text-sm">Scan failed</AlertTitle>
            <AlertDescription className="text-xs">{error}</AlertDescription>
          </Alert>
        )}

        <PopoverList
          processes={displayProcesses}
          rowChanges={rowChanges}
          allowSystemProcessActions={settings.allowSystemProcessActions}
          useHttpsForLocalhost={settings.useHttpsForLocalhost}
          portLookupEmpty={portLookupEmpty}
          exactPortQuery={exactPortQuery}
          loading={loading}
          onStop={handleStop}
        />

        <PopoverFooter
          userCount={userCount}
          loading={refreshing}
          onRefresh={() => void refresh()}
        />
      </div>

      <StopDialog
        processes={stopTarget ? [stopTarget] : []}
        open={stopTarget !== null}
        onOpenChange={(open) => {
          if (!open) {
            setStopTarget(null);
          }
        }}
        title={
          stopTarget && primaryPort(stopTarget) !== null
            ? `Stop ${stopTarget.name} on port ${primaryPort(stopTarget)}?`
            : stopTarget
              ? `Stop ${stopTarget.name}?`
              : undefined
        }
        requireDoubleConfirm={
          stopTarget?.is_system_service === true &&
          settings.allowSystemProcessActions
        }
        onStopped={() => void refresh()}
      />
    </div>
  );
}
