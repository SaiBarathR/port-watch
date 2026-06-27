import { useCallback, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AlertCircleIcon } from "lucide-react";
import { Toaster } from "sonner";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { PortTable } from "@/components/port-table";
import { PortToolbar } from "@/components/port-toolbar";
import { StopDialog } from "@/components/stop-dialog";
import { usePortScan } from "@/hooks/use-port-scan";
import { useLiquidGlass } from "@/hooks/use-liquid-glass";
import { useTheme } from "@/hooks/use-theme";
import type { PortProcess } from "@/lib/types";

function App() {
  const { theme, setTheme, resolvedTheme } = useTheme();
  const {
    processes,
    allProcesses,
    refreshing,
    error,
    refresh,
    search,
    setSearch,
    setSearchField,
    portLookupEmpty,
    exactPortQuery,
    portLookupOccupants,
    settings,
    setHideSystemServices,
    setHideUserServices,
    setAllowSystemProcessActions,
    setRefreshInterval,
    setPreferredEditor,
    setGroupByDirectory,
    setShowChangeToasts,
    togglePinnedPath,
    setWatchedPorts,
    setWatchedPortNotifications,
    setIncludeUdp,
    setUseHttpsForLocalhost,
    setLiquidGlass,
    setGlassTranslucency,
    setGlassBlur,
    setGlassTint,
    setRefreshPaused,
    rowChanges,
    userCount,
    systemCount,
    hiddenSystemCount,
    hiddenUserCount,
  } = usePortScan();

  const [freePortTargets, setFreePortTargets] = useState<PortProcess[]>([]);
  const [freePortNumber, setFreePortNumber] = useState<number | null>(null);

  const handleFreePort = useCallback((port: number, occupants: PortProcess[]) => {
    setFreePortNumber(port);
    setFreePortTargets(occupants);
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    void getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (focused) {
          void refresh();
        }
      })
      .then((fn) => {
        unlisten = fn;
      });

    return () => {
      unlisten?.();
    };
  }, [refresh]);

  useLiquidGlass(settings.liquidGlass, "main", resolvedTheme, {
    translucency: settings.glassTranslucency,
    blur: settings.glassBlur,
    tint: settings.glassTint,
  });

  return (
    <div className="glass-window flex h-screen flex-col bg-background">
      <main className="flex flex-1 flex-col gap-4 overflow-hidden p-4 px-6">
        <PortToolbar
          search={search}
          onSearchChange={setSearch}
          searchField={settings.searchField}
          onSearchFieldChange={setSearchField}
          portLookupEmpty={portLookupEmpty}
          exactPortQuery={exactPortQuery}
          portLookupOccupants={portLookupOccupants}
          exportProcesses={processes}
          settings={settings}
          theme={theme}
          onThemeChange={setTheme}
          onHideSystemChange={setHideSystemServices}
          onHideUserChange={setHideUserServices}
          onAllowSystemActionsChange={setAllowSystemProcessActions}
          onRefreshIntervalChange={setRefreshInterval}
          onPreferredEditorChange={setPreferredEditor}
          onGroupByDirectoryChange={setGroupByDirectory}
          onShowChangeToastsChange={setShowChangeToasts}
          onWatchedPortNotificationsChange={setWatchedPortNotifications}
          onWatchedPortsChange={setWatchedPorts}
          onIncludeUdpChange={setIncludeUdp}
          onUseHttpsForLocalhostChange={setUseHttpsForLocalhost}
          onLiquidGlassChange={setLiquidGlass}
          onGlassTranslucencyChange={setGlassTranslucency}
          onGlassBlurChange={setGlassBlur}
          onGlassTintChange={setGlassTint}
          onFreePort={handleFreePort}
          onRefresh={() => void refresh()}
          loading={refreshing}
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
            search={search}
            settings={settings}
            rowChanges={rowChanges}
            onRefresh={() => void refresh()}
            onRefreshPauseChange={setRefreshPaused}
            onTogglePinnedPath={togglePinnedPath}
            onUseHttpsForLocalhostChange={setUseHttpsForLocalhost}
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
          }
        }}
        title={
          freePortNumber !== null
            ? `Free port ${freePortNumber}?`
            : undefined
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
        onStopped={() => void refresh()}
      />

      <Toaster
        richColors
        closeButton
        expand
        position="bottom-right"
        duration={8000}
        visibleToasts={5}
      />
    </div>
  );
}

export default App;
