import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import type { PortProcess, PreferredEditor } from "@/lib/types";

// The backend's commands, with their arguments spelled once.

/** What tells the process the user saw from a later one under the same PID. */
function seen(process: PortProcess) {
  return {
    pid: process.pid,
    expectedName: process.name,
    expectedStartedAt: process.started_at,
  };
}

export const commands = {
  revealFolder: (path: string) => invoke<void>("open_in_finder", { path }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  openTerminal: (cwd: string) => invoke<void>("open_in_terminal", { cwd }),
  openEditor: (cwd: string, editor: PreferredEditor) =>
    invoke<void>("open_in_editor", { cwd, editor }),
  stopProcess: (process: PortProcess) =>
    invoke<void>("stop_process", seen(process)),
  deleteProject: (
    process: PortProcess,
    path: string,
    mode: "trash" | "permanent",
    confirmation: string | null,
  ) =>
    invoke<void>("delete_project", {
      ...seen(process),
      path,
      mode,
      confirmation,
    }),
};

/**
 * Waits for something the user asked for and shows its failure as a toast,
 * or `done` when it worked. Resolves to whether it worked.
 */
export async function orToast(
  action: Promise<unknown>,
  done?: string,
): Promise<boolean> {
  try {
    await action;
    if (done) {
      toast.success(done);
    }
    return true;
  } catch (err) {
    toast.error(err instanceof Error ? err.message : String(err));
    return false;
  }
}
