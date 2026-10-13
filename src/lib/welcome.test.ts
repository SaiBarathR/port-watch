import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// The guide's state is read once, the first time anything asks. Each test
// loads the module afresh, as a launch does.
const load = () => import("@/lib/welcome");

function storage(initial: Record<string, string> = {}) {
  const kept = new Map(Object.entries(initial));
  return {
    getItem: (key: string) => kept.get(key) ?? null,
    setItem: (key: string, value: string) => void kept.set(key, value),
    removeItem: (key: string) => void kept.delete(key),
  };
}

beforeEach(() => {
  vi.resetModules();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("whether the guide opens with the app", () => {
  it("does until it is asked not to, and again if asked back", async () => {
    vi.stubGlobal("localStorage", storage());
    const { opensWithWelcome, setOpensWithWelcome } = await load();
    expect(opensWithWelcome()).toBe(true);

    setOpensWithWelcome(false);
    expect(opensWithWelcome()).toBe(false);

    setOpensWithWelcome(true);
    expect(opensWithWelcome()).toBe(true);
  });

  // The box that would send it away could not be kept either.
  it("does not where nothing can be stored", async () => {
    const refuse = () => {
      throw new Error("storage is unavailable");
    };
    vi.stubGlobal("localStorage", {
      getItem: refuse,
      setItem: refuse,
      removeItem: refuse,
    });
    const { opensWithWelcome, setOpensWithWelcome } = await load();

    expect(opensWithWelcome()).toBe(false);
    expect(() => setOpensWithWelcome(false)).not.toThrow();
  });
});

describe("the guide", () => {
  it("is up at launch only when it opens with the app", async () => {
    vi.stubGlobal("localStorage", storage());
    expect((await load()).isWelcomeOpen()).toBe(true);

    vi.resetModules();
    const { WELCOME_HIDDEN_KEY } = await load();
    vi.resetModules();
    vi.stubGlobal("localStorage", storage({ [WELCOME_HIDDEN_KEY]: "1" }));
    expect((await load()).isWelcomeOpen()).toBe(false);
  });

  it("stays on its first and last pages when stepped past them", async () => {
    vi.stubGlobal("localStorage", storage());
    const { WELCOME_PAGE_IDS, getWelcome, showWelcome, stepWelcome } =
      await load();
    const page = () => getWelcome().page;
    const first = WELCOME_PAGE_IDS[0];
    const last = WELCOME_PAGE_IDS[WELCOME_PAGE_IDS.length - 1];

    showWelcome();
    stepWelcome(-1);
    expect(page()).toBe(first);

    stepWelcome(1);
    expect(page()).toBe(WELCOME_PAGE_IDS[1]);

    showWelcome(last);
    stepWelcome(1);
    expect(page()).toBe(last);
  });

  it("turns to the page asked for when it is already up", async () => {
    vi.stubGlobal("localStorage", storage());
    const { closeWelcome, getWelcome, showWelcome } = await load();

    showWelcome("stop");
    showWelcome("keys");
    expect(getWelcome()).toEqual({ open: true, page: "keys" });

    // Asked for again without a page, it starts from the first.
    closeWelcome();
    expect(getWelcome().open).toBe(false);
    showWelcome();
    expect(getWelcome()).toEqual({ open: true, page: "list" });
  });
});
