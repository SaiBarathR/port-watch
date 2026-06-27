import { useEffect } from "react";
import {
  GlassMaterialVariant,
  setLiquidGlassEffect,
} from "tauri-plugin-liquid-glass-api";

export function isMacOS(): boolean {
  return navigator.platform.toLowerCase().includes("mac");
}

export type LiquidGlassSurface = "main" | "popover";
export type LiquidGlassTheme = "light" | "dark-grey" | "dark-oled";

export interface LiquidGlassOptions {
  /** How see-through the window is. */
  translucency: "subtle" | "medium" | "clear";
  /** Frost (backdrop blur) strength. */
  blur: "light" | "medium" | "heavy";
  /** Apply a per-theme tint to the native glass (keeps dark themes from washing out). */
  tint: boolean;
}

const SURFACE_CONFIG: Record<
  LiquidGlassSurface,
  { variant: (typeof GlassMaterialVariant)[keyof typeof GlassMaterialVariant]; cornerRadius: number }
> = {
  main: { variant: GlassMaterialVariant.Sidebar, cornerRadius: 0 },
  popover: {
    variant: GlassMaterialVariant.CartouchePopover,
    cornerRadius: 12,
  },
};

// Subtle tint blended into the native glass so dark themes keep contrast and the
// material reads as part of the theme rather than washed-out grey. Format is
// #RRGGBBAA. Light theme stays untinted so it picks up the wallpaper naturally.
const TINT_COLOR: Record<LiquidGlassTheme, string | undefined> = {
  light: undefined,
  "dark-grey": "#1f1f2199",
  "dark-oled": "#0a0a0abf",
};

export function useLiquidGlass(
  enabled: boolean,
  surface: LiquidGlassSurface,
  theme: LiquidGlassTheme,
  options: LiquidGlassOptions,
) {
  const { translucency, blur, tint } = options;

  useEffect(() => {
    if (!isMacOS()) {
      return;
    }

    const root = document.documentElement;

    if (enabled) {
      root.classList.add("liquid-glass");
      // CSS reads these to pick translucency/blur levels (see index.css).
      root.dataset.glassLevel = translucency;
      root.dataset.glassBlur = blur;
      const config = SURFACE_CONFIG[surface];
      void setLiquidGlassEffect({
        enabled: true,
        variant: config.variant,
        cornerRadius: config.cornerRadius,
        tintColor: tint ? TINT_COLOR[theme] : undefined,
      });
    } else {
      root.classList.remove("liquid-glass");
      delete root.dataset.glassLevel;
      delete root.dataset.glassBlur;
      void setLiquidGlassEffect({ enabled: false });
    }

    return () => {
      root.classList.remove("liquid-glass");
      delete root.dataset.glassLevel;
      delete root.dataset.glassBlur;
      void setLiquidGlassEffect({ enabled: false });
    };
  }, [enabled, surface, theme, translucency, blur, tint]);
}
