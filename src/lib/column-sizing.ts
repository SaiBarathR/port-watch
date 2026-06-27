import type { ColumnSizingState } from "@tanstack/react-table";

const COLUMN_SIZING_KEY = "port-watch-column-sizing";

export const DEFAULT_COLUMN_SIZING: ColumnSizingState = {
  select: 40,
  ports: 120,
  name: 100,
  pid: 72,
  user: 96,
  script: 280,
  directory: 200,
  uptime: 88,
  type: 120,
  actions: 52,
};

export function loadColumnSizing(): ColumnSizingState {
  try {
    const raw = localStorage.getItem(COLUMN_SIZING_KEY);
    if (raw) {
      return { ...DEFAULT_COLUMN_SIZING, ...JSON.parse(raw) };
    }
  } catch {
    // ignore
  }
  return DEFAULT_COLUMN_SIZING;
}

export function saveColumnSizing(sizing: ColumnSizingState) {
  localStorage.setItem(COLUMN_SIZING_KEY, JSON.stringify(sizing));
}

export function clampColumnWidth(
  width: number,
  minSize = 20,
  maxSize = Number.MAX_SAFE_INTEGER,
): number {
  return Math.round(Math.min(maxSize, Math.max(minSize, width)));
}

export function totalColumnWidth(sizing: ColumnSizingState): number {
  return Object.values(sizing).reduce((sum, width) => sum + width, 0);
}
