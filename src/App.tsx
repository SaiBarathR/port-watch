import { useCallback, useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AlertCircleIcon } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { AppToaster } from "@/components/app-toaster";
import { PortTable } from "@/components/port-table";
import { PortToolbar } from "@/components/port-toolbar";
import { CliInstallPrompt } from "@/components/cli-install-prompt";
import { ProcessActionsProvider } from "@/components/process-actions";
import { useChangeToastMuteExpiry } from "@/hooks/use-change-toast-mute";
import { usePortQuery } from "@/hooks/use-port-query";
import { useScanSideEffects } from "@/hooks/use-scan-side-effects";
import { useScanStream } from "@/hooks/use-scan-stream";
import { useTheme } from "@/hooks/use-theme";
import { useSettings } from "@/lib/settings-store";

function App() {
  const { theme, setTheme, resolvedTheme } = useTheme();
  const settings = useSettings();
  const { rowChanges, onScanChange } = useScanSideEffects();
  const { processes, loading, refreshing, error, refresh } =
    useScanStream(onScanChange);
  const {
    search,
    setSearch,
    shown,
    exactPortQuery,
    portLookupOccupants,
    portLookupEmpty,
  } = usePortQuery(processes, loading);
  useChangeToastMuteExpiry();

  const systemCount = processes.filter((p) => p.is_system_service).length;
  const userCount = processes.length - systemCount;
  const refreshNow = useCallback(() => void refresh(), [refresh]);

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
      <ProcessActionsProvider processes={processes} onChanged={refreshNow}>
        <main className="flex flex-1 flex-col gap-4 overflow-hidden p-4 px-6">
          <PortToolbar
            search={search}
            onSearchChange={setSearch}
            portLookupEmpty={portLookupEmpty}
            exactPortQuery={exactPortQuery}
            portLookupOccupants={portLookupOccupants}
            shownProcesses={shown}
            settings={settings}
            theme={theme}
            onThemeChange={setTheme}
            onRefresh={refreshNow}
            loading={refreshing}
            firstScanPending={loading}
            userCount={userCount}
            systemCount={systemCount}
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
              processes={processes}
              shownProcesses={shown}
              loading={loading}
              settings={settings}
              rowChanges={rowChanges}
            />
          </div>
        </main>
      </ProcessActionsProvider>

      <CliInstallPrompt />

      <AppToaster theme={resolvedTheme === "light" ? "light" : "dark"} />
    </div>
  );
}

export default App;
