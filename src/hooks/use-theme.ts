import { useCallback, useEffect, useState } from "react";
import { getCurrentWindow, type Theme } from "@tauri-apps/api/window";
import { getPlatform } from "@/lib/platform";

export type ThemeMode = "light" | "dark-grey" | "dark-oled" | "system";
export type ResolvedTheme = "light" | "dark-grey" | "dark-oled";

const THEME_KEY = "port-watch-theme";

export function getStoredTheme(): ThemeMode {
  let value: string | null = null;
  try {
    value = localStorage.getItem(THEME_KEY);
  } catch {
    // storage unavailable — fall through to "system"
  }
  if (value === "dark") {
    return "dark-oled";
  }
  if (
    value === "light" ||
    value === "dark-grey" ||
    value === "dark-oled" ||
    value === "system"
  ) {
    return value;
  }
  return "system";
}

export function resolveTheme(mode: ThemeMode): ResolvedTheme {
  if (mode === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark-grey"
      : "light";
  }
  return mode;
}

let windowTheme: Theme | null | undefined;

// The title bar is the system's to draw, so it is told which theme was
// picked. In "system" mode it is handed back to the OS: a window pinned to
// one theme reports that theme as prefers-color-scheme, whatever the OS says.
//
// Not on Linux. There, handing the window back means "light" rather than
// "whatever the desktop uses", so "system" on a dark desktop would turn the
// app light. The desktop keeps drawing the title bar its own way.
function syncWindowTheme(mode: ThemeMode) {
  if (getPlatform() === "linux") {
    return;
  }
  const theme = mode === "system" ? null : mode === "light" ? "light" : "dark";
  if (theme === windowTheme) {
    return;
  }
  windowTheme = theme;
  try {
    getCurrentWindow()
      .setTheme(theme)
      .catch(() => {});
  } catch {
    // not inside the app window (tests, a plain browser)
  }
}

export function applyTheme(mode: ThemeMode) {
  syncWindowTheme(mode);
  const resolved = resolveTheme(mode);
  document.documentElement.classList.remove("dark-grey", "dark-oled");
  if (resolved === "dark-grey") {
    document.documentElement.classList.add("dark-grey");
  } else if (resolved === "dark-oled") {
    document.documentElement.classList.add("dark-oled");
  }
}

export function initTheme() {
  applyTheme(getStoredTheme());
}

export function useTheme() {
  const [theme, setThemeState] = useState<ThemeMode>(() => getStoredTheme());
  const [resolvedTheme, setResolvedTheme] = useState<ResolvedTheme>(() =>
    resolveTheme(theme),
  );

  const setTheme = useCallback((mode: ThemeMode) => {
    setThemeState(mode);
    setResolvedTheme(resolveTheme(mode));
    try {
      localStorage.setItem(THEME_KEY, mode);
    } catch {
      // storage unavailable — theme still applies for this session
    }
    applyTheme(mode);
  }, []);

  useEffect(() => {
    applyTheme(theme);

    if (theme !== "system") {
      return;
    }

    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => {
      applyTheme("system");
      setResolvedTheme(resolveTheme("system"));
    };
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [theme]);

  return {
    theme,
    setTheme,
    resolvedTheme,
  };
}
