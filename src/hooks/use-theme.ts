import { useCallback, useEffect, useState } from "react";

export type ThemeMode = "light" | "dark-grey" | "dark-oled" | "system";
export type ResolvedTheme = "light" | "dark-grey" | "dark-oled";

const THEME_KEY = "port-watch-theme";

export function getStoredTheme(): ThemeMode {
  const value = localStorage.getItem(THEME_KEY);
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

export function applyTheme(mode: ThemeMode) {
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

  const setTheme = useCallback((mode: ThemeMode) => {
    setThemeState(mode);
    localStorage.setItem(THEME_KEY, mode);
    applyTheme(mode);
  }, []);

  useEffect(() => {
    applyTheme(theme);

    if (theme !== "system") {
      return;
    }

    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => applyTheme("system");
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [theme]);

  return {
    theme,
    setTheme,
    resolvedTheme: resolveTheme(theme),
  };
}
