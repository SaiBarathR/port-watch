import { useCallback, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AlertCircleIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { AppToaster } from "@/components/app-toaster";
import { PortTable } from "@/components/port-table";
import { PortToolbar } from "@/components/port-toolbar";
import { CliInstallPrompt } from "@/components/cli-install-prompt";
import { StopDialog } from "@/components/stop-dialog";
import { usePortScan } from "@/hooks/use-port-scan";
import { useTheme } from "@/hooks/use-theme";
import type { PortProcess } from "@/lib/types";

function App() {
  const { theme, setTheme, resolvedTheme } = useTheme();
  const {
    processes,
    allProcesses,
    loading,
    refreshing,
    error,
    refresh,
    search,
    setSearch,
    portLookupEmpty,
    exactPortQuery,
    portLookupOccupants,
    settings,
    setRefreshPaused,
    rowChanges,
    userCount,
    systemCount,
    hiddenSystemCount,
    hiddenUserCount,
  } = usePortScan();

  const [freePortTargets, setFreePortTargets] = useState<PortProcess[]>([]);
  const [freePortNumber, setFreePortNumber] = useState<number | null>(null);

  const handleFreePort = useCallback(
    (port: number, occupants: PortProcess[]) => {
      setFreePortNumber(port);
      setFreePortTargets(occupants);
      setRefreshPaused(true);
    },
    [setRefreshPaused],
  );

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (focused) {
          void refresh();
        }
      })
      .then((fn) => {
        if (cancelled) {
          fn();
          return;
        }
        unlisten = fn;
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [refresh]);

  return (
    <div className="flex h-screen flex-col bg-background">
      <main className="flex flex-1 flex-col gap-4 overflow-hidden p-4 px-6">
        <PortToolbar
          search={search}
          onSearchChange={setSearch}
          portLookupEmpty={portLookupEmpty}
          exactPortQuery={exactPortQuery}
          portLookupOccupants={portLookupOccupants}
          exportProcesses={processes}
          settings={settings}
          theme={theme}
          onThemeChange={setTheme}
          onFreePort={handleFreePort}
          onRefresh={() => void refresh()}
          loading={refreshing}
          firstScanPending={loading}
          userCount={userCount}
          systemCount={systemCount}
          hiddenSystemCount={hiddenSystemCount}
          hiddenUserCount={hiddenUserCount}
        />

        {error && (
          <Alert variant="destructive">
            <AlertCircleIcon />
            <AlertTitle>Scan failed</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}

        <div className="min-h-0 flex-1">
          <PortTable
            processes={allProcesses}
            loading={loading}
            search={search}
            settings={settings}
            rowChanges={rowChanges}
            onRefresh={() => void refresh()}
            onRefreshPauseChange={setRefreshPaused}
          />
        </div>
      </main>

      <StopDialog
        processes={freePortTargets}
        open={freePortTargets.length > 0 && freePortNumber !== null}
        onOpenChange={(open) => {
          if (!open) {
            setFreePortTargets([]);
            setFreePortNumber(null);
            setRefreshPaused(false);
          }
        }}
        title={
          freePortNumber !== null ? `Free port ${freePortNumber}?` : undefined
        }
        description={
          freePortNumber !== null
            ? `Stop ${freePortTargets.length} process${freePortTargets.length === 1 ? "" : "es"} to free port ${freePortNumber}.`
            : undefined
        }
        requireDoubleConfirm={
          freePortTargets.some((process) => process.is_system_service) &&
          settings.allowSystemProcessActions
        }
        onStopped={() => {
          setRefreshPaused(false);
          void refresh();
        }}
      />

      <CliInstallPrompt />

      <AppToaster theme={resolvedTheme === "light" ? "light" : "dark"} />
    </div>
  );
}

export default App;
