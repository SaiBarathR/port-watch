import { describe, expect, it } from "vitest";
import { formatAgo } from "@/components/refresh-state";

describe("formatAgo", () => {
  it("rounds down to the unit a glance needs", () => {
    expect(formatAgo(0)).toBe("just now");
    expect(formatAgo(4)).toBe("just now");
    expect(formatAgo(5)).toBe("5 s ago");
    expect(formatAgo(59)).toBe("59 s ago");
    expect(formatAgo(60)).toBe("1 min ago");
    expect(formatAgo(3599)).toBe("59 min ago");
    expect(formatAgo(3600)).toBe("1 h ago");
    expect(formatAgo(9000)).toBe("2 h ago");
  });
});
