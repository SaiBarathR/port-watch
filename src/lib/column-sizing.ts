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
    const stored: unknown = JSON.parse(
      localStorage.getItem(COLUMN_SIZING_KEY) ?? "null",
    );
    if (stored && typeof stored === "object" && !Array.isArray(stored)) {
      // Only widths a column can have: anything else would reach the table
      // as a width and leave the column unusable.
      const widths = Object.entries(stored).filter(
        ([, width]) =>
          typeof width === "number" && Number.isFinite(width) && width > 0,
      );
      return { ...DEFAULT_COLUMN_SIZING, ...Object.fromEntries(widths) };
    }
  } catch {
    // ignore
  }
  return DEFAULT_COLUMN_SIZING;
}

export function saveColumnSizing(sizing: ColumnSizingState) {
  try {
    localStorage.setItem(COLUMN_SIZING_KEY, JSON.stringify(sizing));
  } catch {
    // Storage is full or unavailable: the widths last for the session.
  }
}
