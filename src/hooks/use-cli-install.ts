import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import {
  fetchCliInstallStatus,
  installCliToPath,
  uninstallCliFromPath,
  type CliInstallStatus,
} from "@/lib/cli-install";

/**
 * Whether the `port-watch` command is on the PATH, and installing or
 * removing it. Settings and the first-launch banner both use this; each had
 * its own copy.
 */
export function useCliInstall(active = true) {
  const [status, setStatus] = useState<CliInstallStatus | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!active) {
      return;
    }
    let cancelled = false;
    void fetchCliInstallStatus().then((fetched) => {
      if (!cancelled) {
        setStatus(fetched);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [active]);

  const change = useCallback(
    async (action: () => Promise<void>, done: string, failed: string) => {
      setBusy(true);
      try {
        await action();
        setStatus(await fetchCliInstallStatus());
        toast.success(done, {
          description:
            action === installCliToPath
              ? "Run port-watch check 3000 from your terminal."
              : undefined,
        });
        return true;
      } catch (err) {
        toast.error(failed, {
          description: err instanceof Error ? err.message : String(err),
        });
        return false;
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  return {
    status,
    busy,
    install: useCallback(
      () =>
        change(
          installCliToPath,
          "Command-line tool installed",
          "Could not install the command-line tool",
        ),
      [change],
    ),
    uninstall: useCallback(
      () =>
        change(
          uninstallCliFromPath,
          "Command-line tool removed",
          "Could not remove the command-line tool",
        ),
      [change],
    ),
  };
}
