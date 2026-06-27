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

export function isLinux(): boolean {
  return getPlatform() === "linux";
}

export function isWindows(): boolean {
  return getPlatform() === "windows";
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
      return "%LOCALAPPDATA%\\Programs\\Port Watch\\port-watch.exe";
    default:
      return "your PATH";
  }
}

export function cliInstallPrivilegeHint(): string {
  switch (getPlatform()) {
    case "macos":
      return "macOS may ask for your password to write to /usr/local/bin.";
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

export function protectedPathsDescription(): string {
  switch (getPlatform()) {
    case "macos":
      return "Protected system paths under /System, /usr, /bin, /sbin, and /Library cannot be deleted. Paths under /usr/local are allowed.";
    case "linux":
      return "Protected system paths under /usr, /bin, /sbin, /lib, /lib64, and /opt cannot be deleted. Paths under /usr/local are allowed.";
    case "windows":
      return "Protected system paths under Windows, Program Files, and ProgramData cannot be deleted.";
    default:
      return "Protected system paths cannot be deleted.";
  }
}
