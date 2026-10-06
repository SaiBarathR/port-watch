import type { ColumnSizingState } from "@tanstack/react-table";

const COLUMN_SIZING_KEY = "port-watch-column-sizing";

// Together these fit a 1200 px window with every column shown.
export const DEFAULT_COLUMN_SIZING: ColumnSizingState = {
  select: 40,
  ports: 150,
  name: 120,
  pid: 68,
  user: 90,
  script: 240,
  directory: 190,
  uptime: 80,
  type: 110,
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
