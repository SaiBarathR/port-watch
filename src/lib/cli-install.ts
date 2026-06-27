import { invoke } from "@tauri-apps/api/core";

const CLI_PROMPT_DISMISSED_KEY = "port-watch-cli-prompt-dismissed";

export interface CliInstallStatus {
  installed: boolean;
  linkPath: string;
  targetPath: string | null;
  pointsToApp: boolean;
}

export function isCliInstallPromptDismissed(): boolean {
  try {
    return localStorage.getItem(CLI_PROMPT_DISMISSED_KEY) === "1";
  } catch {
    return false;
  }
}

export function dismissCliInstallPrompt(): void {
  try {
    localStorage.setItem(CLI_PROMPT_DISMISSED_KEY, "1");
  } catch {
    // ignore
  }
}

export async function fetchCliInstallStatus(): Promise<CliInstallStatus | null> {
  try {
    return await invoke<CliInstallStatus>("get_cli_install_status");
  } catch {
    return null;
  }
}

export async function installCliToPath(): Promise<void> {
  await invoke("install_cli_to_path");
}

export async function uninstallCliFromPath(): Promise<void> {
  await invoke("uninstall_cli_from_path");
}
