// @vitest-environment jsdom
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS } from "@/lib/types";

const LEGACY_KEY = "port-watch-settings";

// A launch against a backend with no settings file: it has adopted nothing
// yet, and says so. Returns what the window handed over.
async function launchWithoutSettingsFile() {
  const handedOver: unknown[] = [];
  vi.resetModules();
  clearMocks();
  mockIPC(
    (command, payload) => {
      const args = (payload ?? {}) as { legacy?: object };
      if (command === "get_settings") {
        return { settings: DEFAULT_SETTINGS, revision: 1, adopted: false };
      }
      if (command === "adopt_window_settings") {
        handedOver.push(args.legacy);
        return {
          settings: { ...DEFAULT_SETTINGS, ...args.legacy },
          revision: 2,
        };
      }
      return null;
    },
    { shouldMockEvents: true },
  );

  const { initSettings, getSettings } = await import("@/lib/settings-store");
  await initSettings();
  return { handedOver, settings: getSettings() };
}

afterEach(() => {
  clearMocks();
});

describe("the settings the window used to keep", () => {
  it("are handed to the backend on the first launch, and left in place", async () => {
    const legacy = JSON.stringify({ menuBarMode: true, watchedPorts: [3000] });
    localStorage.setItem(LEGACY_KEY, legacy);

    const { handedOver, settings } = await launchWithoutSettingsFile();

    expect(handedOver).toHaveLength(1);
    expect(handedOver[0]).toMatchObject({
      menuBarMode: true,
      watchedPorts: [3000],
    });
    expect(settings.menuBarMode).toBe(true);
    // An older version of the app still reads them from here.
    expect(localStorage.getItem(LEGACY_KEY)).toBe(legacy);
  });

  // The settings file deleted to start afresh, or lost, long after.
  it("are not handed over a second time", async () => {
    localStorage.setItem(LEGACY_KEY, JSON.stringify({ menuBarMode: true }));
    await launchWithoutSettingsFile();

    const { handedOver, settings } = await launchWithoutSettingsFile();

    expect(handedOver).toEqual([{}]);
    expect(settings).toEqual(DEFAULT_SETTINGS);
  });
});
