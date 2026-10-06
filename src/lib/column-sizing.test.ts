// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  DEFAULT_COLUMN_SIZING,
  loadColumnSizing,
  saveColumnSizing,
} from "@/lib/column-sizing";

const KEY = "port-watch-column-sizing";

afterEach(() => {
  vi.restoreAllMocks();
});

describe("column widths", () => {
  it("are the defaults until one is changed", () => {
    expect(loadColumnSizing()).toEqual(DEFAULT_COLUMN_SIZING);
  });

  it("come back as they were saved, the rest at their defaults", () => {
    saveColumnSizing({ ports: 220 });

    expect(loadColumnSizing()).toEqual({
      ...DEFAULT_COLUMN_SIZING,
      ports: 220,
    });
  });

  it("ignore anything stored that is not a width", () => {
    for (const stored of [
      "not json",
      '"wide"',
      "[300, 400]",
      '{"ports":"wide","name":null,"pid":-5,"user":0}',
    ]) {
      localStorage.setItem(KEY, stored);
      expect(loadColumnSizing()).toEqual(DEFAULT_COLUMN_SIZING);
    }

    localStorage.setItem(KEY, '{"ports":"wide","name":200}');
    expect(loadColumnSizing()).toEqual({ ...DEFAULT_COLUMN_SIZING, name: 200 });
  });

  it("last for the session when they cannot be stored", () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("full", "QuotaExceededError");
    });

    expect(() => saveColumnSizing({ ports: 220 })).not.toThrow();
  });
});
