import { useEffect, useSyncExternalStore } from "react";
import { toast } from "sonner";
import {
  fetchCliInstallStatus,
  installCliToPath,
  uninstallCliFromPath,
  type CliInstallStatus,
} from "@/lib/cli-install";

// Whether the `port-watch` command is on the PATH. One copy, for Settings
// and for the strip under the table: with a copy each, installing from one
// left the other still offering to install.
interface CliInstall {
  /** Null until the backend has been asked. */
  status: CliInstallStatus | null;
  busy: boolean;
}

let state: CliInstall = { status: null, busy: false };
const watchers = new Set<() => void>();

function set(next: Partial<CliInstall>) {
  state = { ...state, ...next };
  for (const watcher of watchers) {
    watcher();
  }
}

function subscribe(watcher: () => void) {
  watchers.add(watcher);
  return () => watchers.delete(watcher);
}

async function refresh() {
  set({ status: await fetchCliInstallStatus() });
}

async function change(
  action: () => Promise<void>,
  done: string,
  hint: string | undefined,
  failed: string,
): Promise<boolean> {
  set({ busy: true });
  try {
    await action();
    await refresh();
    toast.success(done, { description: hint });
    return true;
  } catch (err) {
    toast.error(failed, {
      description: err instanceof Error ? err.message : String(err),
    });
    return false;
  } finally {
    set({ busy: false });
  }
}

/** Installs the command. Resolves to whether it worked. */
export function installCli(): Promise<boolean> {
  return change(
    installCliToPath,
    "Command-line tool installed",
    "Run port-watch check 3000 from your terminal.",
    "Could not install the command-line tool",
  );
}

export function uninstallCli(): Promise<boolean> {
  return change(
    uninstallCliFromPath,
    "Command-line tool removed",
    undefined,
    "Could not remove the command-line tool",
  );
}

/**
 * The command's install state. While `active`, the backend is asked afresh
 * each time a component starts using it (the link can change outside the
 * app).
 */
export function useCliInstall(active = true): CliInstall {
  useEffect(() => {
    if (active) {
      void refresh();
    }
  }, [active]);

  return useSyncExternalStore(subscribe, () => state);
}
