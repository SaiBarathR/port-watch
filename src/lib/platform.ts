import { platform } from "@tauri-apps/plugin-os";

export function getPlatform(): "macos" | "linux" | "windows" | "unknown" {
  try {
    const value = platform();
    if (value === "macos" || value === "linux" || value === "windows") {
      return value;
    }
    return "unknown";
  } catch {
    const nav = navigator.platform.toLowerCase();
    if (nav.includes("mac")) return "macos";
    if (nav.includes("win")) return "windows";
    if (nav.includes("linux")) return "linux";
    return "unknown";
  }
}

export function isMacOS(): boolean {
  return getPlatform() === "macos";
}

export function platformLabel(): string {
  switch (getPlatform()) {
    case "macos":
      return "macOS";
    case "linux":
      return "Linux";
    case "windows":
      return "Windows";
    default:
      return "your system";
  }
}

export function cliInstallPathHint(): string {
  switch (getPlatform()) {
    case "macos":
      return "/usr/local/bin/port-watch";
    case "linux":
      return "~/.local/bin/port-watch";
    case "windows":
      return "%LOCALAPPDATA%\\Programs\\Port Watch\\port-watch.cmd";
    default:
      return "your PATH";
  }
}

export function cliInstallPrivilegeHint(): string {
  switch (getPlatform()) {
    case "macos":
      return "macOS asks for an administrator password to change /usr/local/bin.";
    case "linux":
      return "Adds a symlink in ~/.local/bin (ensure it is on your PATH).";
    case "windows":
      return "Adds a shim under LocalAppData and updates your user PATH.";
    default:
      return "";
  }
}

export function systemStopWarning(): string {
  return `Stopping system processes may affect ${platformLabel()} functionality. Confirm again to proceed.`;
}

export function stopProcessDescription(pid: number): string {
  switch (getPlatform()) {
    case "windows":
      return `This stops PID ${pid} via taskkill, forcing termination if needed.`;
    default:
      return `This sends SIGTERM to PID ${pid}, then SIGKILL after 2 seconds if the process is still running.`;
  }
}

export function stopMultipleProcessDescription(): string {
  switch (getPlatform()) {
    case "windows":
      return "Each process is stopped via taskkill, forcing termination if needed.";
    default:
      return "Each process receives SIGTERM, then SIGKILL after 2 seconds if still running.";
  }
}

export function deletableFoldersDescription(): string {
  return "Only project folders inside your home folder can be deleted. Your home folder, its standard folders (Desktop, Documents, Downloads, …) and anything holding app data or settings are protected.";
}
