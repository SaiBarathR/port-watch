import { describe, expect, it } from "vitest";
import { WELCOME_PAGE_IDS } from "@/lib/welcome";
import {
  chordSpoken,
  chordText,
  welcomePages,
  welcomeShortcuts,
  type Platform,
} from "@/lib/welcome-pages";

const PLATFORMS: Platform[] = ["macos", "windows", "linux", "unknown"];
const pagesOn = (platform: Platform) =>
  welcomePages({ platform, editor: "cursor" });

describe("the guide's pages", () => {
  it.each(PLATFORMS)(
    "are the ones the list names, in its order, on %s",
    (platform) => {
      expect(pagesOn(platform).map((page) => page.id)).toEqual([
        ...WELCOME_PAGE_IDS,
      ]);
    },
  );

  // The picture finds a tip's number by its mark.
  it.each(PLATFORMS)("give every tip a mark of its own, on %s", (platform) => {
    for (const page of pagesOn(platform)) {
      const marks = page.tips.map((tip) => tip.mark);
      expect(new Set(marks).size, page.title).toBe(marks.length);
    }
  });

  it.each(["windows", "linux", "unknown"] as const)(
    "say nothing in a Mac's words on %s",
    (platform) => {
      const said = JSON.stringify([
        pagesOn(platform),
        welcomeShortcuts(platform),
      ]);
      expect(said).not.toMatch(/[⌘⌥⇧⌫⌦↵]|Dock|menu bar|Finder|macOS/);
    },
  );

  it("name the editor that projects open in", () => {
    const said = (editor: "cursor" | "code") =>
      JSON.stringify(welcomePages({ platform: "macos", editor }));
    expect(said("cursor")).toContain("**Open in Cursor**");
    expect(said("code")).toContain("**Open in VS Code**");
  });

  it("call the Recycle Bin that on Windows", () => {
    expect(JSON.stringify(pagesOn("windows"))).toContain(
      "**Move to Recycle Bin…**",
    );
    expect(JSON.stringify(pagesOn("linux"))).toContain("**Move to Trash…**");
  });
});

describe("a chord", () => {
  it("is written run together on a Mac and with plus signs elsewhere", () => {
    expect(chordText(["⇧", "⌘", "C"], "macos")).toBe("⇧⌘C");
    expect(chordText(["Ctrl", "Shift", "C"], "windows")).toBe("Ctrl+Shift+C");
  });

  it("is read out by the names of its keys", () => {
    expect(chordSpoken(["⇧", "⌘", "C"])).toBe("Shift Command C");
    expect(chordSpoken(["Ctrl", "Backspace"])).toBe("Ctrl Backspace");
    expect(chordSpoken(["↓"])).toBe("Down Arrow");
  });
});
